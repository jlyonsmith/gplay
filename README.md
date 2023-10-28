# Google Play Tool

[![coverage](https://shields.io/endpoint?url=https://raw.githubusercontent.com/jlyonsmith/gplay/main/coverage.json)](https://github.com/jlyonsmith/gplay/blob/main/coverage.json)
[![Crates.io](https://img.shields.io/crates/v/gplay.svg)](https://crates.io/crates/gplay)
[![Docs.rs](https://docs.rs/gplay/badge.svg)](https://docs.rs/gplay)

This is a tool uploading bundles to the Google Play console from the command line.  It makes use of the [Google Play Developer API](https://developers.google.com/android-publisher/getting_started).

## Introduction

This tool is intended to enable you to automatically upload Android `.aab` bundle files to the Google Playstore as part of your automated build process.  The goal is something similar to the `xcrun altool` tool for iOS. It has some additional features to allow you to list the existing versions and the available test tracks.

Run `gplay --help` to see the available options. After your build completes you'll do something like this:

```sh
gplay upload --cred-file ~/.playstore/your-name-32f41bf78d1a.json --package-name com.your-name.your-app --bundle-
file ./build/app/outputs/bundle/appRelease/app-release.aab --track-name internal
```

The tool uses the simple upload model, so you will need to increase the timeout for large bundle files.

## Setup

This tool uses the Google Play Android Developer API in Google Cloud to upload new bundle builds.  Setting up Google Cloud is a bit overwhelming.

> It is important to note how Google app versioning.  While your app should have a semantic version (major, minor, patch) each build also needs to have a unique integer version number across all releases of the app.  You cannot upload the same build version more than once.  Also, it seems that build numbers can be uploaded in any order, they just need to be unique.  You'll need to figure out how this works in the context of your build system.

Here is a general summary of the steps you will need to take.

1. Go to [Google Accounts](https://myaccount.google.com/) and set up an account
2. Go to the [Google Play Console](https://play.google.com/console) and set up a developer account
3. Create your app, making a note of the *package name*, e.g. com.yourname.yourapp
4. Upload build number `1` manually.  *If anyone from Google is reading this, this is an annoying restriction and should be fixed.*
5. Go to the [Google Cloud Console](https://console.cloud.google.com) and enable the *Google Play Android Developer API*
6. Create a service account in the Google Cloud Console
7. Generate and download a `.json` containing the login credentials.  Put it somewhere safe and `chmod o=` to make sure only you have access.
8. Add the service account as a user in the Play Console. Give it all *Releases* permissions.
9. Test everything out by running a `gplay list-bundles` command.

Once this is done you can use the `upload` sub-command to upload your binaries to publish a new build to a given test track. Then you can go to the Play Console UI and move the build through the release tracks as needed.

## Suggested Enhancements

Pull requests welcome for the following features:

- Support for re-startable uploads
- More Android Publisher API's support
- Refactoring to improve the code
- Support for other methods of authentication
