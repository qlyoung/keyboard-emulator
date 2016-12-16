keyboard-emulator
=================

A USB HID keyboard emulator in Rust.

Usage
-----
```
$ ./keyboard <layout> <script> [output]
```

where
* `<layout>` is a layout file specifying the desired keyboard layout
* `<script>` is an extended DuckyScript script
* `[output]` is an optional output file; the default is stdout

Caveats
-------

* Currently writes errors and debugging output to stdout

License
-------

```
Copyright 2016  Quentin Young

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE.  See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with
this program.  If not, see http://www.gnu.org/licenses/.
```
