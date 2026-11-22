Signed by Leyla Barlak

## Windows launcher publisher pin

On Windows, the launcher verifies its own executable with Authenticode's
[WinVerifyTrust](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrust)
policy. It checks the verified primary signer's full certificate subject and the
SHA-256 fingerprint of its immediate issuing certificate in
`crates/arctic-app/src/build_signature/windows.rs`.

The signed v0.6.6 launcher supplied these values.

- Publisher subject is `C=TR, L=Samsun, O=Open Source Developer, CN=Open Source Developer Leyla Barlak`.
- Issuer is `Certum Code Signing 2021 CA` with SHA-256 fingerprint
  `5AC82CBE9F28351E85D5262293BCFCB8ACABEAD1294F248C1DF17F81CE5AC3CE`.
- The leaf SHA-1 thumbprint is `1CB4E723F6E1CE921BE5C2E6CBB7B22A131B57D4`, as configured in
  `packaging/windows/sign-release.ps1`.

For annual renewal, sign a test executable with the new certificate and run
`signtool verify /pa /v` on Windows. Compare its full subject and immediate
issuer certificate with the pins above. A renewal with the same subject and
issuer certificate only needs a thumbprint update in the signing script.
If either pin changes, confirm the new values against the certificate from
SimplySign, update `PUBLISHER_SUBJECT` (DER-encoded X.500 name) and/or
`ISSUER_SHA256`, and build again before signing the next release. To export the
DER subject from the leaf PEM, use Python's
`cryptography.x509.load_pem_x509_certificate(pem).subject.public_bytes()`.
Use `openssl x509 -in issuer.pem -noout -sha256 -fingerprint` to get the issuer pin.
Launch the newly signed build on Windows and check that About and the main window
show no warning.
