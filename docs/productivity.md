# Local tools, dictation, and meeting recordings

Cikung stores tasks, notes, saved file references, imported lyrics, timer settings,
and hydration counts in its normal local application storage. The file shelf saves
paths, not copies: moving or deleting the original file makes that shelf item
unavailable. Removing a shelf item does not delete the original file.

The focus timer supports a custom 1–240 minute focus or break, pause, reset, and
manual switching between focus and break. A completed session produces one
notification and waits for the next action. Focus statistics count actual timer
time. Restarting Cikung restores the remaining time paused. Hydration reminders
start disabled, support a custom interval and snoozing, and reset the number of
drinks when the computer's local calendar date changes.

Notes accept UTF-8 text or imported transcripts up to 2 MB and export Markdown.
Local task extraction recognizes explicit `- [ ]`, `TODO:`, and `Action:` lines.
It does not infer decisions from a meeting. Cloud summarization/transcription,
when selected in the app, is a separate action.

Lyrics support plain text and LRC, including repeated timestamps, millisecond
precision, Unicode, and the global `[offset:...]` tag. Positive offsets advance
lyrics. Plain text has no timing highlight. Timed lyrics highlight only after
the first timestamp. Lyrics can be saved as LRC or plain text.

## Dictation

Windows dictation starts only after pressing its button. It uses Windows speech
recognition with the selected system speech language and Windows' own listening
interface. Speech language packs, microphone access, and online speech settings
can affect availability. Errors are shown instead of substituting sample text.
Windows may process speech online according to the user's Windows speech
settings. This facility is not a background microphone listener.

An optional local transcription route runs a user-selected **whisper.cpp CLI**
with an explicitly selected audio file and optional local model file. Cikung does
not download an engine or model. Arguments are passed directly to the selected
program, without a command shell. Transcription can be canceled and is stopped
after 30 minutes. The resulting transcript is returned as text. Use a trusted
local executable; selecting it authorizes executing that program.

Native dictation currently targets Windows. Local whisper.cpp transcription and
transcript import provide an alternative on macOS and Linux.

## Meeting recording

Recording is **off until the user presses Start recording**. On Windows, Cikung
requests the default microphone and the default speaker's WASAPI system loopback.
It saves two separate 16-bit PCM WAV files under a new `Cikung-...` folder in the
chosen output folder. The microphone and speaker tracks retain their respective
device sample rates and channel counts. They are not mixed, and exact sample
synchronization is not guaranteed. Silent intervals are represented by zero
samples so that later audio keeps its approximate position in the session.

Pressing Stop finishes the WAV headers. Closing Cikung also requests Stop and
finishes the files. Captures remain local; the recording module neither uploads
audio nor starts transcription. Cloud audio transcription requires the separate
action in the app.

If one endpoint is unavailable, Cikung preserves the other and reports the
missing source after Stop. Disconnects and audio discontinuities are also
reported. A saved track may therefore contain gaps. Recordings stop at two hours
to bound file growth. Available disk space still matters, especially with many
audio channels. This implementation does not isolate a particular call or app:
system loopback includes the sounds sent to the selected default speaker.
Changing output devices during capture can stop that track.

Native microphone/system capture on macOS and Linux is not implemented in this
version. Those platforms report that limitation and support imported audio or
transcripts instead. Unit tests cover synthetic audio conversion and local state;
real microphone, speaker, and Windows speech behavior require device testing.
