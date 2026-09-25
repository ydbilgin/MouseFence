# Development

Barrier logic lives in the pure, Win32-free GuardCore and has regression coverage in tests/Tests.cs.
The app observes mouse input and never changes the display configuration.
All interface text goes through Strings; English and Turkish dictionaries have matching keys.
Keep the application light: no NuGet dependencies, pure WinForms.
Dark theme uses manual Theming and owner-drawn tabs; combo dropdowns and MessageBox staying light is accepted.
Injected input is ignored by the low-level hook; physical cursor behavior needs a manual check.

Run tests: `dotnet run --project tests/MouseFence.Tests.csproj -c Release`. This is a console harness; `dotnet test` is a no-op.
Build: `dotnet build MouseFence.csproj -c Release`. A running application can lock the default output; use `--no-incremental -o <build-directory>` for a separate build.
Run: `dotnet run -c Release`. A running instance holds the single-instance mutex, so a second tray application exits immediately. Use `--screenshot-demo` for UI checks; it runs before the mutex check.

Development screenshots: `MouseFence.exe --screenshot <png> [Light|Dark] [tabIndex] [en|tr]`.
Demo screenshots: `MouseFence.exe --screenshot-demo <png> <Light|Dark> <tab 0/1/2/3, -1 boundary, -2 advanced-fold> <en|tr> [scale] [WxH]`. This uses a synthetic layout with no real hardware exposed.

Publish a small framework-dependent executable: `dotnet publish -c Release -r win-x64 --self-contained false -p:PublishSingleFile=true`.
Publish a standalone executable: `dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:EnableCompressionInSingleFile=true`.

Settings file: `%APPDATA%\MouseFence\settings.json`.
