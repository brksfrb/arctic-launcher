# Verify an Arctic Launcher download

A GitHub attestation records which workflow and source commit built a file.
The Windows downloads are signed after the build. The verification script
makes a copy without the Windows signature and checks its SHA-256 against the
checksum published by the build. Your original download stays intact.

## Windows

1. From the [same release](https://github.com/brksfrb/arctic-launcher/releases),
   download `arctic-launcher-windows-x64.exe` and `SHA256SUMS-unsigned.txt`.
   Keep the original filenames and put both in the same folder.
2. Save [verify-windows.ps1](packaging/verify/verify-windows.ps1) from this
   repository in that folder. Open PowerShell there and run:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File .\verify-windows.ps1 .\arctic-launcher-windows-x64.exe
   ```

3. Install a recent [GitHub CLI](https://cli.github.com/), run `gh auth login`,
   and check the copy produced by the script:

   ```powershell
   gh attestation verify ./arctic-launcher-windows-x64.exe.unsigned --repo brksfrb/arctic-launcher --signer-workflow brksfrb/arctic-launcher/.github/workflows/release.yml
   ```

   Check the source commit in the result. To require a particular commit, add
   `--source-digest` followed by its full commit SHA. Keep the signed download
   for playing. Use `arctic-windows-x64.exe` in the commands to verify the CLI.

On Linux or macOS, install `osslsigncode` and save [verify.sh](packaging/verify/verify.sh)
next to the Windows download and checksum file. Run
`bash ./verify.sh ./arctic-launcher-windows-x64.exe`, then use the same
`gh attestation verify` command. For a Linux or macOS release asset, run the
GitHub CLI command directly on that download.

## v0.6.6 and older

These releases have no attestations or unsigned checksum file. You can compare
a Windows download with the original build while GitHub still keeps the
workflow artifact. On v0.6.6, open PowerShell in the folder containing the signed
launcher and the verification script, then run:

```powershell
gh run download 44717317864 --repo brksfrb/arctic-launcher --name windows --dir ci
Get-ChildItem ./ci/*.exe | ForEach-Object {
    '{0}  {1}' -f (Get-FileHash $_.FullName).Hash.ToLowerInvariant(), $_.Name
} | Set-Content -Encoding ascii ./ci/SHA256SUMS-unsigned.txt
powershell -NoProfile -ExecutionPolicy Bypass -File .\verify-windows.ps1 .\arctic-launcher-windows-x64.exe .\ci\SHA256SUMS-unsigned.txt
```

A matching checksum here establishes that the normalized download matches the
retained CI file. For an earlier version, use that release's workflow run ID.
