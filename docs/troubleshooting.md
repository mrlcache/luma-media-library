# Troubleshooting

[← Back to Luma](../README.md)

## Check the running version

Open Luma through its **Windows Start Menu shortcut**, then open **Settings → Local library**. Version 0.1.3 displays the running desktop version there.

## Missing artwork or title details

Check your internet connection and refresh the library from Settings. A refresh reports metadata errors, including missing TMDb credentials in the current profile. Some titles may have no artwork even when their metadata was matched.

TMDb's read access token is read from the `tmdb_read_access_token` file in the app's data directory. Do not share the token in an issue or commit it to Git.

## Playback cannot start

The Windows installer includes the MPV runtime. Install the full release package when updating; copying only the executable can leave native engines missing.

Version 0.1.3 preserves the native player's error message when startup fails. Include that message and your Luma version in a bug report. The alternative VLC engine requires VLC to be available on your machine.

## Different libraries in development and the installed app

The catalog and metadata configuration use `%APPDATA%\local.media.platform`. An app launched by a packaged development tool can inherit Windows AppData virtualization and read a private profile under that tool's package `LocalCache\Roaming` directory.

Two launches can therefore use the same executable while reading different catalog data. Verify release behavior through the Windows shortcut, including the version shown in Settings and the expected collection.

If two profiles need repair, close the app and make SQLite-consistent backups of both databases before making changes. Match media by the canonical library folder path and the relative file path when transferring metadata: numeric media IDs may differ between profiles. Preserve playback history, and keep credentials and database backups out of Git.
