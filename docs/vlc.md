# VLC on Windows, macOS, and Linux

Cikung first uses the operating system's media interfaces: Windows system media
sessions, Linux MPRIS, or the public macOS player scripting dictionary. VLC must
publish a native session for that route. Some VLC Windows builds do not.

For those builds, Cikung includes an optional **local VLC Web interface** fallback.
It is disabled until you save a VLC password in Cikung Settings. Cikung never
changes VLC's settings automatically.

1. In VLC, open Preferences and show **All** settings.
2. Under **Interface → Main interfaces**, enable **Web**.
3. Under **Interface → Main interfaces → Lua**, set a password for **Lua HTTP**.
   Configure the HTTP host as `127.0.0.1` and port `8080` where your VLC version
   exposes these options. Consult your VLC version's settings if their locations
   differ. Keep the Web interface restricted to your own computer.
4. Save VLC's settings and restart VLC.
5. In Cikung Settings, save the same password under **VLC local interface**.
   Start media playback in VLC and allow up to ten seconds for Cikung to pick up
   a changed password.

The equivalent VLC command-line options, when supported by your installed
version, are `--extraintf=http --http-host=127.0.0.1 --http-port=8080` plus a Lua
HTTP password configured in VLC preferences. Avoid placing your password in a
command line, where it can appear in process lists or command history.

Cikung stores the password in the operating system's credential store under
service `Cikung`, account `service-vlc`. It reads only
`http://127.0.0.1:8080/requests/status.json`, with no proxy, no redirects, and a
400 ms timeout. It does not fetch media URLs or cover URLs returned by VLC.
Removing the saved password disables HTTP discovery on the next credential
refresh. No saved password means no HTTP requests or connection warning.

The fallback reads actual title, artist, album, playback state, position, duration,
shuffle, and repeat flags. It provides play, pause, previous/next, bounded absolute
seek, shuffle, and playlist repeat. Changes to shuffle/repeat use the current
VLC state so pressing a desired state twice does not toggle it back. Repeat
enabled means repeating the playlist; repeat disabled clears both track and
playlist repeat.

If VLC already has a native media session, Cikung skips the HTTP fallback to avoid
showing the same player twice. A configured but unreachable VLC interface produces
a connection error when there are no other working media players; it does not
hide healthy Spotify/browser sessions. A stopped VLC instance with no current
media is omitted from the player list.

Native artwork is currently read from Windows media-session thumbnails only,
cached by track, and limited to 2 MB. Native macOS controls require Automation
permission. Browsers that do not publish media information cannot be detected
through VLC's interface.
