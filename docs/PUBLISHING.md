# Publish ModemDeck Linux to GitHub from Ubuntu

The repository name used by the included scripts is **modemdeck-linux**.

## 1. Install Git and GitHub CLI

```bash
sudo apt update
sudo apt install -y git gh
```

Authenticate once:

```bash
gh auth login
```

Choose GitHub.com → HTTPS → Login with a web browser.

## 2. Publish the repository

From the project root:

```bash
./scripts/publish-github.sh
```

The script creates a public `modemdeck-linux` repository in the currently authenticated GitHub account, pushes `main`, sets the MilMit homepage/description, and adds Linux/modem-related topics.

## 3. Create a GitHub Release with binaries

```bash
./scripts/release-github.sh
```

The script tags the version from `Cargo.toml` (for example `v1.0.0-rc1.4`) and pushes the tag. GitHub Actions then builds the `.deb`, AppImage, source archive and SHA256 checksums and publishes them as a GitHub Release.

You do **not** need to build the release binaries locally for the GitHub release.

## Social preview

Optionally upload the prepared MilMit social-preview image in GitHub repository **Settings → General → Social preview**.
