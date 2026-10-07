# Sign a release's Windows downloads with the Certum code signing certificate, then publish it.
#
# Usage (SimplySign Desktop connected, from the repo):
#     powershell -File packaging\windows\sign-release.ps1 v0.3.0-beta.4
#
# A tag's release is made as a draft by .github/workflows/release.yml (the certificate lives in
# Certum's cloud and needs SimplySign on this PC, so CI can't sign). This downloads the draft's
# files, signs and timestamps the two .exe files, checks the signatures, uploads them over the
# unsigned ones, redoes the checksums (file and notes table) and publishes the release.
param(
	[Parameter(Mandatory = $true)][string]$Tag,
	# The certificate's thumbprint ("Open Source Developer Leyla Barlak", Certum, until 2027-10-07).
	[string]$Thumbprint = "1CB4E723F6E1CE921BE5C2E6CBB7B22A131B57D4",
	[string]$Timestamp = "http://time.certum.pl",
	[string]$SiteRevalidate = "https://arcticlauncher.com/api/revalidate"
)
$ErrorActionPreference = "Stop"

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
	Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signtool) { throw "signtool.exe not found: install the Windows SDK" }
if (-not (Get-ChildItem Cert:\CurrentUser\My | Where-Object Thumbprint -eq $Thumbprint)) {
	throw "Certificate $Thumbprint not found: connect SimplySign Desktop first"
}

$work = Join-Path ([IO.Path]::GetTempPath()) "arctic-sign-$Tag"
if (Test-Path $work) { Remove-Item -Recurse -Force $work }
New-Item -ItemType Directory $work | Out-Null
try {
	gh release download $Tag --dir $work
	if ($LASTEXITCODE -ne 0) { throw "couldn't download the release $Tag" }

	$exes = Get-ChildItem $work -Filter "*windows*.exe"
	if ($exes.Count -eq 0) { throw "the release has no Windows .exe files" }
	foreach ($exe in $exes) {
		& $signtool.FullName sign /sha1 $Thumbprint /tr $Timestamp /td sha256 /fd sha256 /d "Arctic Launcher" $exe.FullName
		if ($LASTEXITCODE -ne 0) { throw "signing $($exe.Name) failed" }
		& $signtool.FullName verify /pa $exe.FullName
		if ($LASTEXITCODE -ne 0) { throw "$($exe.Name) doesn't verify" }
	}

	# Checksums of every download, as the workflow writes them (sha256sum format).
	$sums = Join-Path $work "SHA256SUMS.txt"
	$lines = Get-ChildItem $work -File | Where-Object Name -ne "SHA256SUMS.txt" | Sort-Object Name | ForEach-Object {
		"{0}  {1}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name
	}
	[IO.File]::WriteAllText($sums, ($lines -join "`n") + "`n")

	gh release upload $Tag @($exes.FullName) $sums --clobber
	if ($LASTEXITCODE -ne 0) { throw "uploading the signed files failed" }

	# The notes start with a checksum table: rebuild it for the signed files.
	$table = @("### SHA-256", "", "| File | SHA-256 |", "|---|---|") +
		($lines | ForEach-Object { $hash, $name = $_ -split "  ", 2; "| ``$name`` | ``$hash`` |" })
	$body = gh release view $Tag --json body --jq .body
	$notes = ($body -join "`n") -replace '(?s)### SHA-256.*?(?=\nCheck a download)', (($table -join "`n") + "`n")
	$notesFile = Join-Path $work "notes.md"
	[IO.File]::WriteAllText($notesFile, $notes)
	gh release edit $Tag --notes-file $notesFile --draft=false
	if ($LASTEXITCODE -ne 0) { throw "publishing the release failed" }
	Write-Host "Signed and published $Tag"

	# The website caches GitHub's release data for an hour; ask it to refresh now. The token is
	# kept outside the repos; without it the site catches up on its own within the hour.
	$tokenFile = Join-Path $env:USERPROFILE ".arctic-release\site-revalidate-token"
	if (Test-Path $tokenFile) {
		$token = (Get-Content $tokenFile -Raw).Trim()
		try {
			Invoke-RestMethod -Method Post -Uri $SiteRevalidate -Headers @{ Authorization = "Bearer $token" } | Out-Null
			Write-Host "arcticlauncher.com refreshed"
		} catch {
			Write-Warning "arcticlauncher.com didn't refresh ($($_.Exception.Message)); it catches up within the hour"
		}
	}
} finally {
	Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
