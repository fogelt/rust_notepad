@echo off
rem rmd - open a Markdown file in Rusty Notepad from any cmd window.
rem Usage: rmd notes.md  (also works with no args, or paths with spaces)
rem This file must sit next to rust_notepad.exe; it launches that exe and
rem forwards all arguments. Put its folder on PATH (see scripts/setup-rmd.ps1).
start "Rusty Notepad" "%~dp0rust_notepad.exe" %*
