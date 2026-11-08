# Contributing to MouseFence

Thanks for helping. Bug reports, ideas and pull requests are all welcome. Please be kind and patient in
issues and reviews.

## Build and test

You need Windows and the [.NET 9 SDK](https://dotnet.microsoft.com/download/dotnet/9.0).

```powershell
dotnet run -c Release                                      # run the tray app
dotnet run --project tests/MouseFence.Tests.csproj -c Release   # run the tests
```

The test project is a console program: use `dotnet run`, not `dotnet test`. It prints
`ALL TESTS PASSED` and exits with 0 when everything passes.

A running MouseFence holds the single-instance lock and can lock the default build output; exit it from the
tray, or build into a separate folder with `dotnet build MouseFence.csproj -c Release --no-incremental -o build`.

## Ground rules for changes

- Keep every pointer decision in the pure, Win32-free `GuardCore` (`Core/`), and add or update a scenario in
  `tests/Tests.cs` for any behaviour change.
- MouseFence observes the mouse and never changes the Windows display configuration. Keep it that way.
- All interface text goes through `Strings`; the English and Turkish dictionaries (`I18n/`) must have the same
  keys.
- No NuGet dependencies; plain WinForms.
- The mouse hook ignores simulated input on purpose, so the real pointer feel cannot be tested with fake mouse
  events. When behaviour changes, try it with a real mouse on Windows and say so in the pull request.
- If the UI changes, regenerate the README pictures with `--screenshot-demo` (see the README's
  "Build from source"). More notes: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).
- The README hero, feature table, screenshot gallery and social preview are generated from
  `docs/showcase/showcase.yml`; edit that file and follow [docs/showcase/UPDATING.md](docs/showcase/UPDATING.md).

## Pull requests

1. Fork the repository and create a branch from `main`.
2. Keep the change focused; describe what it changes and how you tested it (tests plus a real-mouse check).
3. Make sure the test program passes before you open the pull request.

Security problems: please do not open a public issue; see [SECURITY.md](SECURITY.md).
