Graphical tool for editing config files for https://github.com/wez/evremap

There exists a tool made in javascript and html, and I wanted a tool that was native and either uses gtk or qt or some other sensible UI framework. And I wanted to learn how to do graphical apps using gtk because I like how it looks.

deps note: GTK 4.18 or higher, targeting gnome api 48 (fedora stable, 42 at the moment)

# Screenshots

![Editor screenshot](https://github.com/user-attachments/assets/6f32d34b-a693-42d4-b6af-1d30f30b3656)
![Device list screenshot](https://github.com/user-attachments/assets/2d5dea8e-fadf-480a-9328-91d03ad206a2)
![Event logger screenshot](https://github.com/user-attachments/assets/323c37f2-7fd5-4c22-995d-c15446ecee68)

# Logging

Set the following environment variables:

- `RUST_LOG=<level>`, where `<level>` is one of "off", "error", "warn", "info", "debug", "trace", case-insensitive and defaults to warn
