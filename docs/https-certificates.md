# HTTPS and private certificate authorities

Ubuntu and the Tizen guest have separate certificate stores. If your company mirror or TLS inspection proxy uses a private certificate authority (CA), obtain its **public root certificate** in PEM format from your IT administrator. Compare its SHA-256 fingerprint with the one provided by IT through a trusted channel.

```sh
openssl x509 -in company-root.pem -noout -subject -issuer -dates -fingerprint -sha256
```

| Where the error occurs | Which trust store to check |
|---|---|
| Downloading the host CLI, resolving a snapshot or fetching build inputs on Ubuntu | Ubuntu, including the Ubuntu distribution running under WSL2 |
| A `zypper` download inside the image build's Tizen chroot or the running VM | Tizen's system CA bundle |
| `tizen-vm update --packages` | Ubuntu fetches repository metadata. The guest downloads RPM payloads. Check which step failed. |
| npm or a Node.js CLI inside Tizen | The guest Node.js process's trusted CAs |

### Ubuntu or WSL2

Install the approved CA in the Ubuntu distribution that runs `tizen-vm`. Ubuntu expects one PEM certificate per file with a `.crt` extension in this directory. Retry the URL that failed after updating the store. [Ubuntu CA installation guide](https://ubuntu.com/server/docs/how-to/security/install-a-root-ca-certificate-in-the-trust-store/)

```sh
sudo install -m 0644 company-root.pem /usr/local/share/ca-certificates/company-root.crt
sudo update-ca-certificates
curl -fsS -o /dev/null https://download.tizen.org/snapshots/TIZEN/Tizen-10.1/Tizen-10.1-Unified/latest/build.xml
```

R5 provisions Ubuntu and Tizen trust separately. If an initial build fails during a Tizen chroot RPM download, use a network or mirror whose TLS chain the stock Tizen image trusts, or provision the approved CA in the staging root filesystem before retrying the build.

### Tizen guest

The R5 system bundle is `/etc/ssl/ca-bundle.pem`, a symlink to `/opt/var/lib/ca-certificates/ca-bundle.pem`. For guest-wide access, place the approved public CA at `/root/company-root.pem`. You can paste its PEM block into that file in the guest root console and check its fingerprint there. Run the following once as guest root, after checking the fingerprint. It keeps the existing roots, file permissions, SMACK attributes and symlink.

```sh
(
    set -e
    ca=/root/company-root.pem
    bundle=$(readlink -f /etc/ssl/ca-bundle.pem)
    if openssl verify -CAfile "$bundle" "$ca" >/dev/null 2>&1; then
        echo 'CA is already trusted'
        exit 0
    fi
    tmp=$(mktemp "$bundle.XXXXXX")
    trap 'rm -f -- "$tmp"' EXIT
    cp --preserve=all "$bundle" "$tmp"
    printf '\n' >> "$tmp"
    openssl x509 -in "$ca" -outform PEM >> "$tmp"
    openssl verify -CAfile "$tmp" "$ca"
    mv -f -- "$tmp" "$bundle"
)
```

Retry the original curl, Git or zypper operation. Keep RPM repository signature verification enabled. A full `tizen-vm update --image` migrates `/root` and installs a fresh system CA bundle. Reapply the approved CA on the new image and recheck it after a CA package upgrade.

For npm and Node-based tools, open `tizen-ai-shell` in the guest. Point new processes at the updated bundle before installing or launching the client. Shell exports apply to commands started in that shell. For an install that needs this setting, run the guest installer there. [Node.js CLI environment variables](https://nodejs.org/api/cli.html#node_extra_ca_certsfile)

```sh
tizen-ai-shell
export NODE_EXTRA_CA_CERTS=/etc/ssl/ca-bundle.pem
npm ping
tizen-ai-install gemini
```

For a single connection with a private CA, you can use `curl --cacert /path/to/company-root.pem URL` or `GIT_SSL_CAINFO=/path/to/company-root.pem git ls-remote URL HEAD`. If a trusted server still fails verification, check `date -u`, the hostname in the URL and the certificate expiry. Ask the server administrator to supply any missing intermediate certificates. [curl TLS certificate guide](https://curl.se/docs/sslcerts.html)
