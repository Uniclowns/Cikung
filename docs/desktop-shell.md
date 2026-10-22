# Desktop tray, notifications, and startup

On Windows, Cikung creates a native notification-area icon from the generated orange cat artwork. Click it to show or hide Cikung. Right-click it for **Show / Hide Cikung**, **Settings**, or **Quit Cikung**. Windows may initially put it in the taskbar's hidden-icons overflow.

The tray has its own message loop and an invisible tool window, so the menu keeps working while the notch is hidden. It restores the icon after Explorer restarts. Exiting removes the icon, destroys its native image and hidden window, and unregisters its window class. Notification and action queues are bounded.

Cikung can send native tray balloon notifications when a focus timer ends or a water reminder is due. Windows notification settings, quiet hours, or Focus Assist can suppress these messages. Cikung uses silent notifications and respects Windows quiet time. Clicking a notification toggles the notch.

**Launch at login** is an opt-in Windows setting. Changing its checkbox adds or removes only the `Cikung` value in your own account's `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` key. It never changes machine-wide startup, needs no administrator privileges, and does not install a service or scheduled task. The value contains the current executable's fully quoted path. Moving a portable Cikung installation requires switching launch at login off and on to update that path. Reading the setting does not change it.

The implementation invokes the Windows registry utility with separate arguments and hides its console window. It does not construct or execute a shell command. Development tests validate path quoting and Unicode notification text; they do not modify your startup settings or create tray notifications.

Native tray and launch-at-login support are currently implemented for Windows. The macOS/Linux versions return an explicit unsupported error for these integrations; they do not claim to create a menu-bar or desktop tray icon.

## Settings window (0.5)

Settings opens a separate, resizable native window in the existing Cikung process. The same window contains General and Cats & appearance. Repeated Settings or Cats requests reuse it. Closing it or pressing Done saves preferences and keeps the top-center notch running. The notch can collapse while Settings remains open.

When media is playing, hover now opens Music automatically. Turn off **Show the music player on hover while music is playing** in General to retain the last feature on hover. Tools in the Music header returns to Overview. This behavior does not replace a tool that is already open.
