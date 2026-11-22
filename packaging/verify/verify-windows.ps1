param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$Checksums
)
$ErrorActionPreference = 'Stop'

$inputFile = Get-Item -LiteralPath $Exe
if (-not $Checksums) {
    $Checksums = Join-Path $inputFile.DirectoryName 'SHA256SUMS-unsigned.txt'
}
$bytes = [IO.File]::ReadAllBytes($inputFile.FullName)
if ($bytes.Length -lt 64 -or [BitConverter]::ToUInt16($bytes, 0) -ne 0x5A4D) {
    throw 'Expected a Windows PE executable.'
}

$pe = [long][BitConverter]::ToUInt32($bytes, 0x3C)
if ($pe -lt 64 -or $pe + 26 -gt $bytes.Length -or [BitConverter]::ToUInt32($bytes, $pe) -ne 0x4550) {
    throw 'Invalid PE header.'
}

$optional = $pe + 24
$optionalEnd = $optional + [BitConverter]::ToUInt16($bytes, ($pe + 20))
switch ([BitConverter]::ToUInt16($bytes, $optional)) {
    0x10B { $directories = $optional + 96 }   # PE32
    0x20B { $directories = $optional + 112 }  # PE32+
    default { throw 'Unsupported PE optional header.' }
}

# The security directory (entry 4) contains a file offset, unlike the other entries.
$security = $directories + 4 * 8
if ($optionalEnd -gt $bytes.Length -or $security + 8 -gt $optionalEnd -or
    [BitConverter]::ToUInt32($bytes, ($directories - 4)) -lt 5) {
    throw 'Missing PE security directory.'
}
$certOffset = [long][BitConverter]::ToUInt32($bytes, $security)
$certSize = [long][BitConverter]::ToUInt32($bytes, ($security + 4))
if ($certOffset -lt $optionalEnd -or $certSize -lt 8 -or $certOffset + $certSize -ne $bytes.Length) {
    throw 'Expected an Authenticode certificate table at the end of the executable.'
}

[Array]::Clear($bytes, $security, 8)
[Array]::Clear($bytes, ($optional + 64), 4)  # PE checksum
# Hash and write only the prefix before the certificate table.
$sha = [Security.Cryptography.SHA256]::Create()
try {
    $hash = [BitConverter]::ToString($sha.ComputeHash($bytes, 0, $certOffset)).Replace('-', '').ToLowerInvariant()
} finally {
    $sha.Dispose()
}
Write-Host "Normalized SHA-256: $hash"

$name = [regex]::Escape($inputFile.Name)
$expected = @(foreach ($line in Get-Content -LiteralPath $Checksums) {
    if ($line -cmatch "^([0-9A-Fa-f]{64}) [ *]$name$") { $Matches[1] }
})
if ($expected.Count -ne 1) { throw "Expected one checksum for $($inputFile.Name) in $Checksums." }
if ($hash -ne $expected[0]) { throw "Checksum mismatch for $($inputFile.Name)." }

$normalized = $inputFile.FullName + '.unsigned'
$output = [IO.File]::Open($normalized, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
try {
    $output.Write($bytes, 0, $certOffset)
} finally {
    $output.Dispose()
}
Write-Host 'Checksum matches. Verify the GitHub attestation with:'
$quoted = $normalized.Replace("'", "''")
Write-Host "gh attestation verify '$quoted' --repo brksfrb/arctic-launcher"
