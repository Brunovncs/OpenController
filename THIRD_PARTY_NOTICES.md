# Third-party notices

OpenController is MIT licensed (see [LICENSE](LICENSE)). Its programs include the code below,
each under its own license. ViGEmBus, HidHide, DsHidMini and BthPS3 are not included: Open
Controller downloads their official installers from Nefarius' GitHub releases when you ask it to.
Neither are usbip-win2 and VIIPER, which the experimental VIIPER option downloads the same way.

## SDL 3

[SDL](https://github.com/libsdl-org/SDL) is compiled into `open-controller.exe` and linked
statically. It is under the zlib license:

> Copyright (C) 1997-2026 Sam Lantinga <slouken@libsdl.org>
>
> This software is provided 'as-is', without any express or implied warranty. In no event will
> the authors be held liable for any damages arising from the use of this software.
>
> Permission is granted to anyone to use this software for any purpose, including commercial
> applications, and to alter it and redistribute it freely, subject to the following
> restrictions:
>
> 1. The origin of this software must not be misrepresented; you must not claim that you wrote
>    the original software. If you use this software in a product, an acknowledgment in the
>    product documentation would be appreciated but is not required.
> 2. Altered source versions must be plainly marked as such, and must not be misrepresented as
>    being the original software.
> 3. This notice may not be removed or altered from any source distribution.

## SDL_GameControllerDB

The Windows entries of [SDL_GameControllerDB](https://github.com/mdqinc/SDL_GameControllerDB)
are bundled in `open-controller.exe`, under the same zlib license as SDL.

## Controller drawings

The DualShock 4 and DualShock 3 drawings in `assets/pads.json` are traced from "Dualshock 4
Layout.svg" and "Dualshock3 Layout.svg" by Tokyoship on Wikimedia Commons, under
[CC BY 3.0](https://creativecommons.org/licenses/by/3.0/); the Joy-Con, from "Nintendo Switch
Joy-Con illustration.svg", in the public domain. `scripts/pads/README.md` lists every source.

## GamepadMotionHelpers

The way OpenController works out a DualShock 3's tilt from its accelerometer (`motion::TiltRate`)
follows the sensor fusion step of [GamepadMotionHelpers](https://github.com/JibbSmart/GamepadMotionHelpers),
under the MIT license:

> Copyright (c) 2020-2023 Julian "Jibb" Smart
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
> associated documentation files (the "Software"), to deal in the Software without restriction,
> including without limitation the rights to use, copy, modify, merge, publish, distribute,
> sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
> NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
> OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## VIIPER

[VIIPER](https://github.com/Alia5/VIIPER) is an experimental option on Windows, off by default.
Its server, `viiper.exe`, is not part of OpenController. When you choose VIIPER and click
Install, OpenController downloads the official v0.8.2 release from GitHub, checks it against
the hash it had when this version was made and unpacks it next to itself, with the
`licenses.txt` it comes with. It runs as a separate program that OpenController talks to over a
local TCP connection; OpenController includes none of its code and stays MIT.

> VIIPER - Virtual Input over IP EmulatoR
>
> Copyright (C) 2025-2026 Peter Repukat
>
> This program is free software: you can redistribute it and/or modify it under the terms of the
> GNU General Public License as published by the Free Software Foundation, either version 3 of
> the License, or (at your option) any later version.

The source of that version is at https://github.com/Alia5/VIIPER/tree/v0.8.2 and the license at
https://www.gnu.org/licenses/gpl-3.0.html. [usbip-win2](https://github.com/vadimgrn/usbip-win2),
the driver VIIPER needs, is installed with its own installer, under its own license.

## Rust crates

Each crate's license text is in its source, at the repository listed. Where a crate offers a
choice of licenses, OpenController uses it under the first one that applies.

| Crate | Version | License |
|---|---|---|
| [accesskit](https://github.com/AccessKit/accesskit) | 0.24.1 | MIT OR Apache-2.0 |
| [accesskit_atspi_common](https://github.com/AccessKit/accesskit) | 0.19.1 | MIT OR Apache-2.0 |
| [accesskit_consumer](https://github.com/AccessKit/accesskit) | 0.38.0 | MIT OR Apache-2.0 |
| [accesskit_macos](https://github.com/AccessKit/accesskit) | 0.26.3 | MIT OR Apache-2.0 |
| [accesskit_unix](https://github.com/AccessKit/accesskit) | 0.22.1 | MIT OR Apache-2.0 |
| [accesskit_windows](https://github.com/AccessKit/accesskit) | 0.34.0 | MIT OR Apache-2.0 |
| [addr2line](https://github.com/gimli-rs/addr2line) | 0.25.1 | Apache-2.0 OR MIT |
| [adler2](https://github.com/oyvindln/adler2) | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| [aes](https://github.com/RustCrypto/block-ciphers) | 0.8.4 | MIT OR Apache-2.0 |
| [ahash](https://github.com/tkaitchuck/ahash) | 0.8.12 | MIT OR Apache-2.0 |
| [aho-corasick](https://github.com/BurntSushi/aho-corasick) | 1.1.5 | Unlicense OR MIT |
| [aligned](https://github.com/rust-embedded-community/aligned) | 0.4.3 | MIT OR Apache-2.0 |
| [aligned-vec](https://github.com/sarah-ek/aligned-vec/) | 0.6.4 | MIT |
| [allocator-api2](https://github.com/zakarumych/allocator-api2) | 0.2.21 | MIT OR Apache-2.0 |
| [anyhow](https://github.com/dtolnay/anyhow) | 1.0.104 | MIT OR Apache-2.0 |
| [arg_enum_proc_macro](https://github.com/lu-zero/arg_enum_proc_macro) | 0.3.4 | MIT |
| [arrayref](https://github.com/droundy/arrayref) | 0.3.9 | BSD-2-Clause |
| [arrayvec](https://github.com/bluss/arrayvec) | 0.7.8 | MIT OR Apache-2.0 |
| [as-raw-xcb-connection](https://github.com/psychon/as-raw-xcb-connection) | 1.0.1 | MIT OR Apache-2.0 |
| [as-slice](https://github.com/japaric/as-slice) | 0.2.1 | MIT OR Apache-2.0 |
| [ash](https://github.com/ash-rs/ash) | 0.38.0+1.3.281 | MIT OR Apache-2.0 |
| [ashpd](https://github.com/bilelmoussaoui/ashpd) | 0.13.13 | MIT |
| [async-broadcast](https://github.com/smol-rs/async-broadcast) | 0.7.2 | MIT OR Apache-2.0 |
| [async-channel](https://github.com/smol-rs/async-channel) | 2.5.0 | Apache-2.0 OR MIT |
| [async-compression](https://github.com/Nullus157/async-compression) | 0.4.50 | MIT OR Apache-2.0 |
| [async-executor](https://github.com/smol-rs/async-executor) | 1.14.0 | Apache-2.0 OR MIT |
| [async-fs](https://github.com/smol-rs/async-fs) | 2.2.0 | Apache-2.0 OR MIT |
| [async-io](https://github.com/smol-rs/async-io) | 2.6.0 | Apache-2.0 OR MIT |
| [async-lock](https://github.com/smol-rs/async-lock) | 3.4.2 | Apache-2.0 OR MIT |
| [async-net](https://github.com/smol-rs/async-net) | 2.0.0 | Apache-2.0 OR MIT |
| [async-process](https://github.com/smol-rs/async-process) | 2.5.0 | Apache-2.0 OR MIT |
| [async-recursion](https://github.com/dcchut/async-recursion) | 1.2.0 | MIT OR Apache-2.0 |
| [async-signal](https://github.com/smol-rs/async-signal) | 0.2.14 | Apache-2.0 OR MIT |
| [async-task](https://github.com/smol-rs/async-task) | 4.7.1 | Apache-2.0 OR MIT |
| [async-trait](https://github.com/dtolnay/async-trait) | 0.1.92 | MIT OR Apache-2.0 |
| [atomic](https://github.com/Amanieu/atomic-rs) | 0.5.3 | Apache-2.0 OR MIT |
| [atomic-waker](https://github.com/smol-rs/atomic-waker) | 1.1.2 | Apache-2.0 OR MIT |
| [atspi](https://github.com/odilia-app/atspi) | 0.29.0 | Apache-2.0 OR MIT |
| [atspi-common](https://github.com/odilia-app/atspi) | 0.13.0 | Apache-2.0 OR MIT |
| [atspi-proxies](https://github.com/odilia-app/atspi) | 0.13.0 | Apache-2.0 OR MIT |
| [av-scenechange](https://github.com/rust-av/av-scenechange) | 0.14.1 | MIT |
| [av1-grain](https://github.com/rust-av/av1-grain) | 0.2.5 | BSD-2-Clause |
| [avif-serialize](https://github.com/kornelski/avif-serialize) | 0.8.9 | BSD-3-Clause |
| [backtrace](https://github.com/rust-lang/backtrace-rs) | 0.3.76 | MIT OR Apache-2.0 |
| [base64](https://github.com/marshallpierce/rust-base64) | 0.22.1 | MIT OR Apache-2.0 |
| [bit-set](https://github.com/contain-rs/bit-set) | 0.9.1 | Apache-2.0 OR MIT |
| [bit-vec](https://github.com/contain-rs/bit-vec) | 0.9.1 | Apache-2.0 OR MIT |
| [bit_field](https://github.com/phil-opp/rust-bit-field) | 0.10.3 | Apache-2.0 OR MIT |
| [bitflags](https://github.com/bitflags/bitflags) | 1.3.2 | MIT OR Apache-2.0 |
| [bitflags](https://github.com/bitflags/bitflags) | 2.13.2 | MIT OR Apache-2.0 |
| [bitstream-io](https://github.com/tuffy/bitstream-io) | 4.10.0 | MIT OR Apache-2.0 |
| [block](http://github.com/SSheldon/rust-block) | 0.1.6 | MIT |
| [block-buffer](https://github.com/RustCrypto/utils) | 0.10.4 | MIT OR Apache-2.0 |
| [block-padding](https://github.com/RustCrypto/utils) | 0.3.3 | MIT OR Apache-2.0 |
| [block2](https://github.com/madsmtm/objc2) | 0.5.1 | MIT |
| [block2](https://github.com/madsmtm/objc2) | 0.6.2 | MIT |
| [blocking](https://github.com/smol-rs/blocking) | 1.7.0 | Apache-2.0 OR MIT |
| [borsh](https://github.com/near/borsh-rs) | 1.8.1 | MIT OR Apache-2.0 |
| [bumpalo](https://github.com/fitzgen/bumpalo) | 3.20.3 | MIT OR Apache-2.0 |
| [bytemuck](https://github.com/Lokathor/bytemuck) | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| [bytemuck_derive](https://github.com/Lokathor/bytemuck) | 1.12.1 | Zlib OR Apache-2.0 OR MIT |
| [byteorder](https://github.com/BurntSushi/byteorder) | 1.5.0 | Unlicense OR MIT |
| [byteorder-lite](https://github.com/image-rs/byteorder-lite) | 0.1.0 | Unlicense OR MIT |
| [bytes](https://github.com/tokio-rs/bytes) | 1.12.1 | MIT |
| [bzip2](https://github.com/trifectatechfoundation/bzip2-rs) | 0.6.1 | MIT OR Apache-2.0 |
| [calloop](https://github.com/Smithay/calloop) | 0.14.5 | MIT |
| [calloop-wayland-source](https://github.com/smithay/calloop-wayland-source) | 0.4.1 | MIT |
| [cbc](https://github.com/RustCrypto/block-modes) | 0.1.2 | MIT OR Apache-2.0 |
| [cfg-if](https://github.com/rust-lang/cfg-if) | 1.0.5 | MIT OR Apache-2.0 |
| [cgl](https://github.com/servo/cgl-rs) | 0.3.2 | MIT  OR  Apache-2.0 |
| [chrono](https://github.com/chronotope/chrono) | 0.4.45 | MIT OR Apache-2.0 |
| [cipher](https://github.com/RustCrypto/traits) | 0.4.4 | MIT OR Apache-2.0 |
| [cocoa](https://github.com/servo/core-foundation-rs) | 0.26.1 | MIT OR Apache-2.0 |
| [cocoa-foundation](https://github.com/servo/core-foundation-rs) | 0.2.1 | MIT OR Apache-2.0 |
| [codespan-reporting](https://github.com/brendanzab/codespan) | 0.13.1 | Apache-2.0 |
| [color_quant](https://github.com/image-rs/color_quant.git) | 1.1.0 | MIT |
| [compression-codecs](https://github.com/Nullus157/async-compression) | 0.4.45 | MIT OR Apache-2.0 |
| [compression-core](https://github.com/Nullus157/async-compression) | 0.4.33 | MIT OR Apache-2.0 |
| [concurrent-queue](https://github.com/smol-rs/concurrent-queue) | 2.5.0 | Apache-2.0 OR MIT |
| [const-random](https://github.com/tkaitchuck/constrandom) | 0.1.18 | MIT OR Apache-2.0 |
| [const-random-macro](https://github.com/tkaitchuck/constrandom) | 0.1.16 | MIT OR Apache-2.0 |
| [convert_case](https://github.com/rutrum/convert-case) | 0.10.0 | MIT |
| [core-foundation](https://github.com/servo/core-foundation-rs) | 0.10.1 | MIT OR Apache-2.0 |
| [core-foundation-sys](https://github.com/servo/core-foundation-rs) | 0.8.7 | MIT OR Apache-2.0 |
| [core-graphics](https://github.com/servo/core-foundation-rs) | 0.24.0 | MIT OR Apache-2.0 |
| [core-graphics-types](https://github.com/servo/core-foundation-rs) | 0.2.0 | MIT OR Apache-2.0 |
| [core-graphics2](https://github.com/rust-media/apple-media-rs) | 0.5.2 | MIT OR Apache-2.0 |
| [core-text](https://github.com/servo/core-foundation-rs) | 21.0.0 | MIT OR Apache-2.0 |
| [core-video](https://github.com/rust-media/apple-media-rs) | 0.5.2 | MIT OR Apache-2.0 |
| [core_detect](https://github.com/thomcc/core_detect) | 1.0.0 | MIT OR Apache-2.0 |
| [core_maths](https://github.com/robertbastian/core_maths) | 0.1.1 | MIT |
| [cosmic-text](https://github.com/pop-os/cosmic-text) | 0.19.0 | MIT OR Apache-2.0 |
| [cpufeatures](https://github.com/RustCrypto/utils) | 0.2.17 | MIT OR Apache-2.0 |
| [crc32fast](https://github.com/srijs/rust-crc32fast) | 1.5.2 | MIT OR Apache-2.0 |
| [crossbeam-channel](https://github.com/crossbeam-rs/crossbeam) | 0.5.17 | MIT OR Apache-2.0 |
| [crossbeam-deque](https://github.com/crossbeam-rs/crossbeam) | 0.8.8 | MIT OR Apache-2.0 |
| [crossbeam-epoch](https://github.com/crossbeam-rs/crossbeam) | 0.9.21 | MIT OR Apache-2.0 |
| [crossbeam-queue](https://github.com/crossbeam-rs/crossbeam) | 0.3.14 | MIT OR Apache-2.0 |
| [crossbeam-utils](https://github.com/crossbeam-rs/crossbeam) | 0.8.23 | MIT OR Apache-2.0 |
| [crunchy](https://github.com/eira-fransham/crunchy) | 0.2.4 | MIT |
| [crypto-common](https://github.com/RustCrypto/traits) | 0.1.7 | MIT OR Apache-2.0 |
| [ctor](https://github.com/mmastrac/linktime) | 1.0.13 | Apache-2.0 OR MIT |
| [data-url](https://github.com/servo/rust-url) | 0.3.2 | MIT OR Apache-2.0 |
| [derive_more](https://github.com/JelteF/derive_more) | 2.1.1 | MIT |
| [derive_more-impl](https://github.com/JelteF/derive_more) | 2.1.1 | MIT |
| [digest](https://github.com/RustCrypto/traits) | 0.10.7 | MIT OR Apache-2.0 |
| [dirs](https://github.com/soc/dirs-rs) | 5.0.1 | MIT OR Apache-2.0 |
| [dirs](https://github.com/soc/dirs-rs) | 6.0.0 | MIT OR Apache-2.0 |
| [dirs-sys](https://github.com/dirs-dev/dirs-sys-rs) | 0.4.1 | MIT OR Apache-2.0 |
| [dirs-sys](https://github.com/dirs-dev/dirs-sys-rs) | 0.5.0 | MIT OR Apache-2.0 |
| [dispatch2](https://github.com/madsmtm/objc2) | 0.3.1 | Zlib OR Apache-2.0 OR MIT |
| [displaydoc](https://github.com/yaahc/displaydoc) | 0.2.7 | MIT OR Apache-2.0 |
| [dlib](https://github.com/elinorbgr/dlib) | 0.5.3 | MIT |
| [document-features](https://github.com/slint-ui/document-features) | 0.2.12 | MIT OR Apache-2.0 |
| [downcast-rs](https://github.com/marcianx/downcast-rs) | 1.2.1 | MIT OR Apache-2.0 |
| [dpi](https://github.com/rust-windowing/winit) | 0.1.2 | Apache-2.0 AND MIT |
| [dunce](https://gitlab.com/kornelski/dunce) | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| [dyn-clone](https://github.com/dtolnay/dyn-clone) | 1.0.20 | MIT OR Apache-2.0 |
| [either](https://github.com/rayon-rs/either) | 1.18.0 | MIT OR Apache-2.0 |
| [encoding_rs](https://github.com/hsivonen/encoding_rs) | 0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause |
| [endi](https://github.com/zeenix/endi) | 1.1.1 | MIT |
| [enumflags2](https://github.com/meithecatte/enumflags2) | 0.7.12 | MIT OR Apache-2.0 |
| [enumflags2_derive](https://github.com/meithecatte/enumflags2) | 0.7.12 | MIT OR Apache-2.0 |
| [enumn](https://github.com/dtolnay/enumn) | 0.1.14 | MIT OR Apache-2.0 |
| [equator](https://github.com/sarah-ek/equator/) | 0.4.2 | MIT |
| [equator-macro](https://github.com/sarah-ek/equator/) | 0.4.2 | MIT |
| [equivalent](https://github.com/indexmap-rs/equivalent) | 1.0.2 | Apache-2.0 OR MIT |
| [erased-serde](https://github.com/dtolnay/erased-serde) | 0.4.10 | MIT OR Apache-2.0 |
| [errno](https://github.com/lambda-fairy/rust-errno) | 0.3.14 | MIT OR Apache-2.0 |
| [etagere](https://github.com/nical/etagere) | 0.2.15 | MIT OR Apache-2.0 |
| [euclid](https://github.com/servo/euclid) | 0.22.14 | MIT OR Apache-2.0 |
| [event-listener](https://github.com/smol-rs/event-listener) | 5.4.2 | Apache-2.0 OR MIT |
| [event-listener-strategy](https://github.com/smol-rs/event-listener-strategy) | 0.5.4 | Apache-2.0 OR MIT |
| [exr](https://github.com/johannesvollmer/exrs) | 1.74.2 | BSD-3-Clause |
| [fastrand](https://github.com/smol-rs/fastrand) | 2.5.0 | Apache-2.0 OR MIT |
| [fax](https://github.com/pdf-rs/fax) | 0.2.7 | MIT |
| [fdeflate](https://github.com/image-rs/fdeflate) | 0.3.7 | MIT OR Apache-2.0 |
| [filedescriptor](https://github.com/wezterm/wezterm) | 0.8.3 | MIT |
| [fixedbitset](https://github.com/petgraph/fixedbitset) | 0.5.7 | MIT OR Apache-2.0 |
| [flate2](https://github.com/rust-lang/flate2-rs) | 1.1.10 | MIT OR Apache-2.0 |
| [float-cmp](https://github.com/mikedilger/float-cmp) | 0.9.0 | MIT |
| [float-ord](https://github.com/notriddle/rust-float-ord) | 0.3.2 | MIT  OR  Apache-2.0 |
| [float_next_after](https://gitlab.com/bronsonbdevost/next_afterf) | 1.0.0 | MIT |
| [flume](https://github.com/zesterer/flume) | 0.12.0 | Apache-2.0 OR MIT |
| [foldhash](https://github.com/orlp/foldhash) | 0.1.5 | Zlib |
| [foldhash](https://github.com/orlp/foldhash) | 0.2.0 | Zlib |
| [font-types](https://github.com/googlefonts/fontations) | 0.11.3 | MIT OR Apache-2.0 |
| [font-types](https://github.com/googlefonts/fontations) | 0.12.6 | MIT OR Apache-2.0 |
| [fontconfig-parser](https://github.com/Riey/fontconfig-parser) | 0.5.8 | MIT |
| [fontdb](https://github.com/RazrFalcon/fontdb) | 0.23.0 | MIT |
| [foreign-types](https://github.com/sfackler/foreign-types) | 0.5.0 | MIT OR Apache-2.0 |
| [foreign-types-macros](https://github.com/sfackler/foreign-types) | 0.2.4 | MIT OR Apache-2.0 |
| [foreign-types-shared](https://github.com/sfackler/foreign-types) | 0.3.1 | MIT OR Apache-2.0 |
| [form_urlencoded](https://github.com/servo/rust-url) | 1.2.2 | MIT OR Apache-2.0 |
| [freetype-sys](https://github.com/PistonDevelopers/freetype-sys.git) | 0.20.1 | MIT |
| [futures](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 |
| [futures-channel](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 |
| [futures-concurrency](https://github.com/yoshuawuyts/futures-concurrency) | 7.7.1 | MIT OR Apache-2.0 |
| [futures-core](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 |
| [futures-executor](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 |
| [futures-io](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 |
| [futures-lite](https://github.com/smol-rs/futures-lite) | 2.6.1 | Apache-2.0 OR MIT |
| [futures-macro](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 |
| [futures-sink](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 |
| [futures-task](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 |
| [futures-util](https://github.com/rust-lang/futures-rs) | 0.3.34 | MIT OR Apache-2.0 |
| [generic-array](https://github.com/fizyk20/generic-array.git) | 0.14.7 | MIT |
| [gethostname](https://codeberg.org/swsnr/gethostname.rs.git) | 1.1.0 | Apache-2.0 |
| [getrandom](https://github.com/rust-random/getrandom) | 0.2.17 | MIT OR Apache-2.0 |
| [getrandom](https://github.com/rust-random/getrandom) | 0.3.4 | MIT OR Apache-2.0 |
| [getrandom](https://github.com/rust-random/getrandom) | 0.4.3 | MIT OR Apache-2.0 |
| [gif](https://github.com/image-rs/image-gif) | 0.14.2 | MIT OR Apache-2.0 |
| [gimli](https://github.com/gimli-rs/gimli) | 0.32.3 | MIT OR Apache-2.0 |
| [glow](https://github.com/grovesNL/glow) | 0.17.0 | MIT OR Apache-2.0 OR Zlib |
| [gpu-allocator](https://github.com/Traverse-Research/gpu-allocator) | 0.28.0 | MIT OR Apache-2.0 |
| [gpu-descriptor](https://github.com/zakarumych/gpu-descriptor) | 0.3.2 | MIT OR Apache-2.0 |
| [gpu-descriptor-types](https://github.com/zakarumych/gpu-descriptor) | 0.2.0 | MIT OR Apache-2.0 |
| [gpui-pre](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-apple](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-collections](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-derive-refineable](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-http-client](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-linux](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-macos](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-macros](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-perf](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-platform](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-refineable](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-scheduler](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-shared-string](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-sum-tree](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-util](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-util-macros](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-wgpu](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-windows](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-zlog](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-ztracing](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-ztracing-macro](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [half](https://github.com/VoidStarKat/half-rs) | 2.7.1 | MIT OR Apache-2.0 |
| [harfrust](https://github.com/harfbuzz/harfrust) | 0.5.2 | MIT |
| [hash32](https://github.com/japaric/hash32) | 0.3.1 | MIT OR Apache-2.0 |
| [hashbrown](https://github.com/rust-lang/hashbrown) | 0.14.5 | MIT OR Apache-2.0 |
| [hashbrown](https://github.com/rust-lang/hashbrown) | 0.15.5 | MIT OR Apache-2.0 |
| [hashbrown](https://github.com/rust-lang/hashbrown) | 0.16.1 | MIT OR Apache-2.0 |
| [hashbrown](https://github.com/rust-lang/hashbrown) | 0.17.1 | MIT OR Apache-2.0 |
| [heapless](https://github.com/rust-embedded/heapless) | 0.9.3 | MIT OR Apache-2.0 |
| [heck](https://github.com/withoutboats/heck) | 0.5.0 | MIT OR Apache-2.0 |
| [hex](https://github.com/KokaKiwi/rust-hex) | 0.4.3 | MIT OR Apache-2.0 |
| [hexf-parse](https://github.com/lifthrasiir/hexf) | 0.2.1 | CC0-1.0 |
| [hkdf](https://github.com/RustCrypto/KDFs/) | 0.12.4 | MIT OR Apache-2.0 |
| [hmac](https://github.com/RustCrypto/MACs) | 0.12.1 | MIT OR Apache-2.0 |
| [http](https://github.com/hyperium/http) | 1.5.0 | MIT OR Apache-2.0 |
| [http-body](https://github.com/hyperium/http-body) | 1.1.0 | MIT |
| [iana-time-zone](https://github.com/strawlab/iana-time-zone) | 0.1.65 | MIT OR Apache-2.0 |
| [icu_collections](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 |
| [icu_locale_core](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 |
| [icu_normalizer](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 |
| [icu_normalizer_data](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 |
| [icu_properties](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 |
| [icu_properties_data](https://github.com/unicode-org/icu4x) | 2.3.0 | Unicode-3.0 |
| [icu_provider](https://github.com/unicode-org/icu4x) | 2.3.1 | Unicode-3.0 |
| [idna](https://github.com/servo/rust-url/) | 1.1.0 | MIT OR Apache-2.0 |
| [idna_adapter](https://github.com/hsivonen/idna_adapter) | 1.2.2 | Apache-2.0 OR MIT |
| [image](https://github.com/image-rs/image) | 0.25.10 | MIT OR Apache-2.0 |
| [image-webp](https://github.com/image-rs/image-webp) | 0.2.4 | MIT OR Apache-2.0 |
| [imagesize](https://github.com/Roughsketch/imagesize) | 0.14.0 | MIT |
| [imgref](https://github.com/kornelski/imgref) | 1.12.3 | CC0-1.0 OR Apache-2.0 |
| [indexmap](https://github.com/indexmap-rs/indexmap) | 2.14.2 | Apache-2.0 OR MIT |
| [inout](https://github.com/RustCrypto/utils) | 0.1.4 | MIT OR Apache-2.0 |
| [inventory](https://github.com/dtolnay/inventory) | 0.3.24 | MIT OR Apache-2.0 |
| [io-surface](https://github.com/servo/core-foundation-rs) | 0.16.1 | MIT OR Apache-2.0 |
| [is-docker](https://github.com/TheLarkInn/is-docker) | 0.2.0 | MIT |
| [is-wsl](https://github.com/TheLarkInn/is-wsl) | 0.4.0 | MIT |
| [itertools](https://github.com/rust-itertools/itertools) | 0.14.0 | MIT OR Apache-2.0 |
| [itoa](https://github.com/dtolnay/itoa) | 1.0.18 | MIT OR Apache-2.0 |
| [keyboard-types](https://github.com/rust-windowing/keyboard-types) | 0.8.3 | MIT OR Apache-2.0 |
| [khronos-egl](https://github.com/timothee-haudebourg/khronos-egl) | 6.0.0 | MIT OR Apache-2.0 |
| [kurbo](https://github.com/linebender/kurbo) | 0.13.1 | Apache-2.0 OR MIT |
| [lazy_static](https://github.com/rust-lang-nursery/lazy-static.rs) | 1.5.1 | MIT OR Apache-2.0 |
| [leak](https://github.com/jmesmon/leak.git) | 0.1.2 | Apache-2.0 OR MIT |
| [leaky-cow](https://github.com/notriddle/rust-leaky-cow) | 0.1.1 | MIT  OR  Apache-2.0 |
| [lebe](https://github.com/johannesvollmer/lebe) | 0.5.3 | BSD-3-Clause |
| [libbz2-rs-sys](https://github.com/trifectatechfoundation/libbzip2-rs) | 0.2.5 | bzip2-1.0.6 |
| [libc](https://github.com/rust-lang/libc) | 0.2.190 | MIT OR Apache-2.0 |
| [libloading](https://github.com/nagisa/rust_libloading/) | 0.8.9 | ISC |
| [libm](https://github.com/rust-lang/compiler-builtins) | 0.2.16 | MIT |
| [linebender_resource_handle](https://github.com/linebender/raw_resource_handle) | 0.1.1 | Apache-2.0 OR MIT |
| [link-section](https://github.com/mmastrac/linktime) | 0.19.3 | Apache-2.0 OR MIT |
| [linktime-proc-macro](https://github.com/mmastrac/linktime) | 0.2.3 | Apache-2.0 OR MIT |
| [linux-raw-sys](https://github.com/sunfishcode/linux-raw-sys) | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| [litemap](https://github.com/unicode-org/icu4x) | 0.8.3 | Unicode-3.0 |
| [litrs](https://github.com/LukasKalbertodt/litrs) | 1.0.0 | MIT OR Apache-2.0 |
| [lock_api](https://github.com/Amanieu/parking_lot) | 0.4.14 | MIT OR Apache-2.0 |
| [log](https://github.com/rust-lang/log) | 0.4.34 | MIT OR Apache-2.0 |
| [loop9](https://gitlab.com/kornelski/loop9.git) | 0.1.5 | MIT |
| [lyon](https://github.com/nical/lyon) | 1.0.19 | MIT OR Apache-2.0 |
| [lyon_algorithms](https://github.com/nical/lyon) | 1.0.21 | MIT OR Apache-2.0 |
| [lyon_geom](https://github.com/nical/lyon) | 1.0.19 | MIT OR Apache-2.0 |
| [lyon_path](https://github.com/nical/lyon) | 1.0.19 | MIT OR Apache-2.0 |
| [lyon_tessellation](https://github.com/nical/lyon) | 1.0.22 | MIT OR Apache-2.0 |
| [mach2](https://github.com/JohnTitor/mach2) | 0.5.0 | BSD-2-Clause OR MIT OR Apache-2.0 |
| [malloc_buf](https://github.com/SSheldon/malloc_buf) | 0.0.6 | MIT |
| [maybe-rayon](https://github.com/shssoichiro/maybe-rayon) | 0.1.1 | MIT |
| [md-5](https://github.com/RustCrypto/hashes) | 0.10.6 | MIT OR Apache-2.0 |
| [memchr](https://github.com/BurntSushi/memchr) | 2.8.3 | Unlicense OR MIT |
| [memmap2](https://github.com/RazrFalcon/memmap2-rs) | 0.9.11 | MIT OR Apache-2.0 |
| [metal](https://github.com/gfx-rs/metal-rs) | 0.33.0 | MIT OR Apache-2.0 |
| [miniz_oxide](https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide) | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| [miniz_oxide](https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide) | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| [moxcms](https://github.com/awxkee/moxcms.git) | 0.8.1 | BSD-3-Clause OR Apache-2.0 |
| [muda](https://github.com/tauri-apps/muda) | 0.21.0 | Apache-2.0 OR MIT |
| [multiversion_no_op](https://github.com/hsivonen/multiversion_no_op) | 1.0.0 | Apache-2.0 OR MIT |
| [naga](https://github.com/gfx-rs/wgpu) | 29.0.4 | MIT OR Apache-2.0 |
| [new_debug_unreachable](https://github.com/mbrubeck/rust-debug-unreachable) | 1.0.6 | MIT |
| [no_std_io2](https://github.com/wcampbell0x2a/no-std-io2) | 0.9.4 | Apache-2.0 OR MIT |
| [nom](https://github.com/rust-bakery/nom) | 8.0.0 | MIT |
| [noop_proc_macro](https://github.com/lu-zero/noop_proc_macro) | 0.3.0 | MIT |
| [notify-rust](https://github.com/hoodie/notify-rust) | 4.18.1 | MIT OR Apache-2.0 |
| [ntapi](https://github.com/MSxDOS/ntapi) | 0.4.3 | Apache-2.0 OR MIT |
| [nu-ansi-term](https://github.com/nushell/nu-ansi-term) | 0.50.3 | MIT |
| [num](https://github.com/rust-num/num) | 0.4.3 | MIT OR Apache-2.0 |
| [num-bigint](https://github.com/rust-num/num-bigint) | 0.4.8 | MIT OR Apache-2.0 |
| [num-bigint-dig](https://github.com/dignifiedquire/num-bigint) | 0.9.1 | MIT OR Apache-2.0 |
| [num-complex](https://github.com/rust-num/num-complex) | 0.4.6 | MIT OR Apache-2.0 |
| [num-derive](https://github.com/rust-num/num-derive) | 0.4.2 | MIT OR Apache-2.0 |
| [num-integer](https://github.com/rust-num/num-integer) | 0.1.47 | MIT OR Apache-2.0 |
| [num-iter](https://github.com/rust-num/num-iter) | 0.1.46 | MIT OR Apache-2.0 |
| [num-rational](https://github.com/rust-num/num-rational) | 0.4.2 | MIT OR Apache-2.0 |
| [num-traits](https://github.com/rust-num/num-traits) | 0.2.19 | MIT OR Apache-2.0 |
| [num_cpus](https://github.com/seanmonstar/num_cpus) | 1.17.0 | MIT OR Apache-2.0 |
| [objc](http://github.com/SSheldon/rust-objc) | 0.2.7 | MIT |
| [objc-sys](https://github.com/madsmtm/objc2) | 0.3.5 | MIT |
| [objc2](https://github.com/madsmtm/objc2) | 0.5.3 | MIT |
| [objc2](https://github.com/madsmtm/objc2) | 0.6.5 | MIT |
| [objc2-app-kit](https://github.com/madsmtm/objc2) | 0.2.2 | MIT |
| [objc2-app-kit](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-core-audio](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-core-audio-types](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-core-data](https://github.com/madsmtm/objc2) | 0.2.2 | MIT |
| [objc2-core-foundation](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-core-graphics](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-core-image](https://github.com/madsmtm/objc2) | 0.2.2 | MIT |
| [objc2-core-location](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-core-media](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-core-video](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-encode](https://github.com/madsmtm/objc2) | 4.1.0 | MIT |
| [objc2-foundation](https://github.com/madsmtm/objc2) | 0.2.2 | MIT |
| [objc2-foundation](https://github.com/madsmtm/objc2) | 0.3.2 | MIT |
| [objc2-io-surface](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-metal](https://github.com/madsmtm/objc2) | 0.2.2 | MIT |
| [objc2-quartz-core](https://github.com/madsmtm/objc2) | 0.2.2 | MIT |
| [objc2-screen-capture-kit](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-security](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-service-management](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc2-user-notifications](https://github.com/madsmtm/objc2) | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| [objc_exception](http://github.com/SSheldon/rust-objc-exception) | 0.1.2 | MIT |
| [object](https://github.com/gimli-rs/object) | 0.37.3 | Apache-2.0 OR MIT |
| [once_cell](https://github.com/matklad/once_cell) | 1.21.4 | MIT OR Apache-2.0 |
| [oo7](https://github.com/linux-credentials/oo7) | 0.6.0 | MIT |
| [open](https://github.com/Byron/open-rs) | 5.4.4 | MIT |
| [option-ext](https://github.com/soc/option-ext.git) | 0.2.0 | MPL-2.0 |
| [ordered-float](https://github.com/reem/rust-ordered-float) | 5.5.0 | MIT |
| [ordered-stream](https://github.com/danieldg/ordered-stream) | 0.2.0 | MIT OR Apache-2.0 |
| [parking](https://github.com/smol-rs/parking) | 2.2.1 | Apache-2.0 OR MIT |
| [parking_lot](https://github.com/Amanieu/parking_lot) | 0.12.5 | MIT OR Apache-2.0 |
| [parking_lot_core](https://github.com/Amanieu/parking_lot) | 0.9.12 | MIT OR Apache-2.0 |
| [paste](https://github.com/dtolnay/paste) | 1.0.15 | MIT OR Apache-2.0 |
| [pastey](https://github.com/as1100k/pastey) | 0.1.1 | MIT OR Apache-2.0 |
| [pathfinder_geometry](https://github.com/servo/pathfinder) | 0.5.1 | MIT OR Apache-2.0 |
| [pathfinder_simd](https://github.com/servo/pathfinder) | 0.5.6 | MIT OR Apache-2.0 |
| [pbkdf2](https://github.com/RustCrypto/password-hashes/tree/master/pbkdf2) | 0.12.2 | MIT OR Apache-2.0 |
| [percent-encoding](https://github.com/servo/rust-url/) | 2.3.2 | MIT OR Apache-2.0 |
| [phf](https://github.com/rust-phf/rust-phf) | 0.13.1 | MIT |
| [phf_generator](https://github.com/rust-phf/rust-phf) | 0.13.1 | MIT |
| [phf_macros](https://github.com/rust-phf/rust-phf) | 0.13.1 | MIT |
| [phf_shared](https://github.com/rust-phf/rust-phf) | 0.13.1 | MIT |
| [pico-args](https://github.com/RazrFalcon/pico-args) | 0.5.0 | MIT |
| [pin-project](https://github.com/taiki-e/pin-project) | 1.1.13 | Apache-2.0 OR MIT |
| [pin-project-internal](https://github.com/taiki-e/pin-project) | 1.1.13 | Apache-2.0 OR MIT |
| [pin-project-lite](https://github.com/taiki-e/pin-project-lite) | 0.2.17 | Apache-2.0 OR MIT |
| [piper](https://github.com/smol-rs/piper) | 0.2.5 | MIT OR Apache-2.0 |
| [png](https://github.com/image-rs/image-png) | 0.17.16 | MIT OR Apache-2.0 |
| [png](https://github.com/image-rs/image-png) | 0.18.1 | MIT OR Apache-2.0 |
| [polling](https://github.com/smol-rs/polling) | 3.11.0 | Apache-2.0 OR MIT |
| [pollster](https://github.com/zesterer/pollster) | 0.2.5 | Apache-2.0 OR MIT |
| [pollster](https://github.com/zesterer/pollster) | 0.4.0 | Apache-2.0 OR MIT |
| [polycool](https://github.com/linebender/kurbo) | 0.4.0 | MIT OR Apache-2.0 |
| [postage](https://github.com/austinjones/postage-rs) | 0.5.0 | MIT |
| [potential_utf](https://github.com/unicode-org/icu4x) | 0.1.6 | Unicode-3.0 |
| [ppv-lite86](https://github.com/cryptocorrosion/cryptocorrosion) | 0.2.21 | MIT OR Apache-2.0 |
| [presser](https://github.com/EmbarkStudios/presser) | 0.3.1 | MIT OR Apache-2.0 |
| [proc-macro-crate](https://github.com/bkchr/proc-macro-crate) | 3.5.0 | MIT OR Apache-2.0 |
| [proc-macro2](https://github.com/dtolnay/proc-macro2) | 1.0.107 | MIT OR Apache-2.0 |
| [profiling](https://github.com/aclysma/profiling) | 1.0.18 | MIT OR Apache-2.0 |
| [profiling-procmacros](https://github.com/aclysma/profiling) | 1.0.18 | MIT OR Apache-2.0 |
| [pulp](https://github.com/sarah-quinones/pulp/) | 0.22.3 | MIT |
| [pulp-wasm-simd-flag](https://github.com/sarah-quinones/pulp/) | 0.1.1 | MIT |
| [pxfm](https://github.com/awxkee/pxfm) | 0.1.30 | BSD-3-Clause OR Apache-2.0 |
| [qoi](https://github.com/aldanor/qoi-rust) | 0.4.1 | MIT OR Apache-2.0 |
| [quick-error](http://github.com/tailhook/quick-error) | 2.0.1 | MIT OR Apache-2.0 |
| [quick-xml](https://github.com/tafia/quick-xml) | 0.41.0 | MIT |
| [quote](https://github.com/dtolnay/quote) | 1.0.47 | MIT OR Apache-2.0 |
| [rand](https://github.com/rust-random/rand) | 0.8.8 | MIT OR Apache-2.0 |
| [rand](https://github.com/rust-random/rand) | 0.9.5 | MIT OR Apache-2.0 |
| [rand_chacha](https://github.com/rust-random/rand) | 0.3.1 | MIT OR Apache-2.0 |
| [rand_chacha](https://github.com/rust-random/rand) | 0.9.0 | MIT OR Apache-2.0 |
| [rand_core](https://github.com/rust-random/rand) | 0.6.4 | MIT OR Apache-2.0 |
| [rand_core](https://github.com/rust-random/rand) | 0.9.5 | MIT OR Apache-2.0 |
| [rangemap](https://github.com/jeffparsons/rangemap) | 1.8.0 | MIT OR Apache-2.0 |
| [rav1e](https://github.com/xiph/rav1e/) | 0.8.1 | BSD-2-Clause |
| [ravif](https://github.com/kornelski/cavif-rs) | 0.13.0 | BSD-3-Clause |
| [raw-cpuid](https://github.com/gz/rust-cpuid) | 11.6.0 | MIT |
| [raw-window-handle](https://github.com/rust-windowing/raw-window-handle) | 0.6.2 | MIT OR Apache-2.0 OR Zlib |
| [rayon](https://github.com/rayon-rs/rayon) | 1.12.0 | MIT OR Apache-2.0 |
| [rayon-core](https://github.com/rayon-rs/rayon) | 1.13.0 | MIT OR Apache-2.0 |
| [read-fonts](https://github.com/googlefonts/fontations) | 0.37.0 | MIT OR Apache-2.0 |
| [read-fonts](https://github.com/googlefonts/fontations) | 0.41.0 | MIT OR Apache-2.0 |
| [reborrow](https://github.com/sarah-ek/reborrow/) | 0.5.5 | MIT |
| [ref-cast](https://github.com/dtolnay/ref-cast) | 1.0.27 | MIT OR Apache-2.0 |
| [ref-cast-impl](https://github.com/dtolnay/ref-cast) | 1.0.27 | MIT OR Apache-2.0 |
| [regex](https://github.com/rust-lang/regex) | 1.13.1 | MIT OR Apache-2.0 |
| [regex-automata](https://github.com/rust-lang/regex) | 0.4.18 | MIT OR Apache-2.0 |
| [regex-syntax](https://github.com/rust-lang/regex) | 0.8.11 | MIT OR Apache-2.0 |
| [renderdoc-sys](https://github.com/ebkalderon/renderdoc-rs) | 1.1.0 | MIT OR Apache-2.0 |
| [resvg](https://github.com/linebender/resvg) | 0.46.0 | Apache-2.0 OR MIT |
| [rgb](https://github.com/kornelski/rust-rgb) | 0.8.53 | MIT |
| [roxmltree](https://github.com/RazrFalcon/roxmltree) | 0.20.0 | MIT OR Apache-2.0 |
| [roxmltree](https://github.com/RazrFalcon/roxmltree) | 0.21.1 | MIT OR Apache-2.0 |
| [rustc-demangle](https://github.com/rust-lang/rustc-demangle) | 0.1.28 | MIT OR Apache-2.0 |
| [rustc-hash](https://github.com/rust-lang-nursery/rustc-hash) | 1.1.0 | Apache-2.0 OR MIT |
| [rustc-hash](https://github.com/rust-lang/rustc-hash) | 2.1.3 | Apache-2.0 OR MIT |
| [rustix](https://github.com/bytecodealliance/rustix) | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| [rustybuzz](https://github.com/harfbuzz/rustybuzz) | 0.20.1 | MIT |
| [ryu](https://github.com/dtolnay/ryu) | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| [same-file](https://github.com/BurntSushi/same-file) | 1.0.6 | Unlicense OR MIT |
| [schemars](https://github.com/GREsau/schemars) | 1.2.2 | MIT |
| [schemars_derive](https://github.com/GREsau/schemars) | 1.2.2 | MIT |
| [scoped-tls](https://github.com/alexcrichton/scoped-tls) | 1.0.1 | MIT OR Apache-2.0 |
| [scopeguard](https://github.com/bluss/scopeguard) | 1.2.0 | MIT OR Apache-2.0 |
| [sdl3-sys](https://codeberg.org/maia/sdl3-sys-rs) | 0.7.2+SDL-3.4.18 | Zlib |
| [seahash](https://gitlab.redox-os.org/redox-os/seahash) | 4.1.0 | MIT |
| [self_cell](https://github.com/Voultapher/self_cell) | 1.3.0 | Apache-2.0 OR GPL-2.0-only |
| [semver](https://github.com/dtolnay/semver) | 1.0.28 | MIT OR Apache-2.0 |
| [serde](https://github.com/serde-rs/serde) | 1.0.229 | MIT OR Apache-2.0 |
| [serde_bytes](https://github.com/serde-rs/bytes) | 0.11.19 | MIT OR Apache-2.0 |
| [serde_core](https://github.com/serde-rs/serde) | 1.0.229 | MIT OR Apache-2.0 |
| [serde_derive](https://github.com/serde-rs/serde) | 1.0.229 | MIT OR Apache-2.0 |
| [serde_derive_internals](https://github.com/serde-rs/serde) | 0.30.0 | MIT OR Apache-2.0 |
| [serde_fmt](https://github.com/KodrAus/serde_fmt.git) | 1.1.0 | Apache-2.0 OR MIT |
| [serde_json](https://github.com/serde-rs/json) | 1.0.151 | MIT OR Apache-2.0 |
| [serde_repr](https://github.com/dtolnay/serde-repr) | 0.1.21 | MIT OR Apache-2.0 |
| [serde_urlencoded](https://github.com/nox/serde_urlencoded) | 0.7.1 | MIT OR Apache-2.0 |
| [sha1_smol](https://github.com/mitsuhiko/sha1-smol) | 1.0.1 | BSD-3-Clause |
| [sha2](https://github.com/RustCrypto/hashes) | 0.10.9 | MIT OR Apache-2.0 |
| [sharded-slab](https://github.com/hawkw/sharded-slab) | 0.1.7 | MIT |
| [signal-hook-registry](https://github.com/vorner/signal-hook) | 1.4.8 | MIT OR Apache-2.0 |
| [simd-adler32](https://github.com/mcountryman/simd-adler32) | 0.3.10 | MIT |
| [simd_helpers](https://github.com/lu-zero/simd_helpers) | 0.1.0 | MIT |
| [simdutf8](https://github.com/rusticstuff/simdutf8) | 0.1.5 | MIT OR Apache-2.0 |
| [simplecss](https://github.com/linebender/simplecss) | 0.2.2 | Apache-2.0 OR MIT |
| [siphasher](https://github.com/jedisct1/rust-siphash) | 1.0.4 | MIT OR Apache-2.0 |
| [skrifa](https://github.com/googlefonts/fontations) | 0.40.0 | MIT OR Apache-2.0 |
| [skrifa](https://github.com/googlefonts/fontations) | 0.44.0 | MIT OR Apache-2.0 |
| [slab](https://github.com/tokio-rs/slab) | 0.4.12 | MIT |
| [slotmap](https://github.com/orlp/slotmap) | 1.1.1 | Zlib |
| [smallvec](https://github.com/servo/rust-smallvec) | 1.16.2 | MIT OR Apache-2.0 |
| [smol](https://github.com/smol-rs/smol) | 2.0.2 | Apache-2.0 OR MIT |
| [smol_str](https://github.com/rust-lang/rust-analyzer/tree/master/lib/smol_str) | 0.3.6 | MIT OR Apache-2.0 |
| [spin](https://github.com/mvdnes/spin-rs.git) | 0.10.1 | MIT |
| [spin](https://github.com/mvdnes/spin-rs.git) | 0.9.9 | MIT |
| [spirv](https://github.com/gfx-rs/rspirv) | 0.4.0+sdk-1.4.341.0 | Apache-2.0 |
| [stable_deref_trait](https://github.com/storyyeller/stable_deref_trait) | 1.2.1 | MIT OR Apache-2.0 |
| [static_assertions](https://github.com/nvzqz/static-assertions-rs) | 1.1.0 | MIT OR Apache-2.0 |
| [strict-num](https://github.com/RazrFalcon/strict-num) | 0.1.1 | MIT |
| [strum](https://github.com/Peternator7/strum) | 0.28.0 | MIT |
| [strum_macros](https://github.com/Peternator7/strum) | 0.28.0 | MIT |
| [subtle](https://github.com/dalek-cryptography/subtle) | 2.6.1 | BSD-3-Clause |
| [sval](https://github.com/sval-rs/sval) | 2.22.0 | Apache-2.0 OR MIT |
| [sval_buffer](https://github.com/sval-rs/sval) | 2.22.0 | Apache-2.0 OR MIT |
| [sval_dynamic](https://github.com/sval-rs/sval) | 2.22.0 | Apache-2.0 OR MIT |
| [sval_fmt](https://github.com/sval-rs/sval) | 2.22.0 | Apache-2.0 OR MIT |
| [sval_json](https://github.com/sval-rs/sval) | 2.22.0 | Apache-2.0 OR MIT |
| [sval_nested](https://github.com/sval-rs/sval) | 2.22.0 | Apache-2.0 OR MIT |
| [sval_ref](https://github.com/sval-rs/sval) | 2.22.0 | Apache-2.0 OR MIT |
| [sval_serde](https://github.com/sval-rs/sval) | 2.22.0 | Apache-2.0 OR MIT |
| [svg_fmt](https://github.com/nical/rust_debug) | 0.4.5 | MIT OR Apache-2.0 |
| [svgtypes](https://github.com/linebender/svgtypes) | 0.16.1 | Apache-2.0 OR MIT |
| [swash](https://github.com/dfrg/swash) | 0.2.10 | Apache-2.0 OR MIT |
| [syn](https://github.com/dtolnay/syn) | 2.0.119 | MIT OR Apache-2.0 |
| [syn](https://github.com/dtolnay/syn) | 3.0.6 | MIT OR Apache-2.0 |
| [synstructure](https://github.com/mystor/synstructure) | 0.14.0 | MIT |
| [sys-locale](https://github.com/1Password/sys-locale) | 0.3.2 | MIT OR Apache-2.0 |
| [sysinfo](https://github.com/GuillaumeGomez/sysinfo) | 0.31.4 | MIT |
| [taffy](https://github.com/DioxusLabs/taffy) | 0.13.0 | MIT |
| [termcolor](https://github.com/BurntSushi/termcolor) | 1.4.1 | Unlicense OR MIT |
| [thiserror](https://github.com/dtolnay/thiserror) | 1.0.69 | MIT OR Apache-2.0 |
| [thiserror](https://github.com/dtolnay/thiserror) | 2.0.21 | MIT OR Apache-2.0 |
| [thiserror-impl](https://github.com/dtolnay/thiserror) | 1.0.69 | MIT OR Apache-2.0 |
| [thiserror-impl](https://github.com/dtolnay/thiserror) | 2.0.21 | MIT OR Apache-2.0 |
| [thread_local](https://github.com/Amanieu/thread_local-rs) | 1.1.10 | MIT OR Apache-2.0 |
| [tiff](https://github.com/image-rs/image-tiff) | 0.11.3 | MIT |
| tiny-keccak | 2.0.2 | CC0-1.0 |
| [tiny-skia](https://github.com/RazrFalcon/tiny-skia) | 0.11.4 | BSD-3-Clause |
| [tiny-skia-path](https://github.com/RazrFalcon/tiny-skia/tree/master/path) | 0.11.4 | BSD-3-Clause |
| [tinystr](https://github.com/unicode-org/icu4x) | 0.8.4 | Unicode-3.0 |
| [tinyvec](https://github.com/Lokathor/tinyvec) | 1.13.3 | Zlib OR Apache-2.0 OR MIT |
| [toml_datetime](https://github.com/toml-rs/toml) | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 |
| [toml_edit](https://github.com/toml-rs/toml) | 0.25.15+spec-1.1.0 | MIT OR Apache-2.0 |
| [toml_parser](https://github.com/toml-rs/toml) | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 |
| [tracing](https://github.com/tokio-rs/tracing) | 0.1.44 | MIT |
| [tracing-attributes](https://github.com/tokio-rs/tracing) | 0.1.31 | MIT |
| [tracing-core](https://github.com/tokio-rs/tracing) | 0.1.36 | MIT |
| [tracing-log](https://github.com/tokio-rs/tracing) | 0.2.0 | MIT |
| [tracing-subscriber](https://github.com/tokio-rs/tracing) | 0.3.23 | MIT |
| [tray-icon](https://github.com/tauri-apps/tray-icon) | 0.26.0 | MIT OR Apache-2.0 |
| [ttf-parser](https://github.com/harfbuzz/ttf-parser) | 0.25.1 | MIT OR Apache-2.0 |
| [typeid](https://github.com/dtolnay/typeid) | 1.0.3 | MIT OR Apache-2.0 |
| [typenum](https://github.com/paholg/typenum) | 1.20.1 | MIT OR Apache-2.0 |
| [unicode-bidi](https://github.com/servo/unicode-bidi) | 0.3.18 | MIT OR Apache-2.0 |
| [unicode-bidi-mirroring](https://github.com/RazrFalcon/unicode-bidi-mirroring) | 0.4.0 | MIT OR Apache-2.0 |
| [unicode-ccc](https://github.com/RazrFalcon/unicode-ccc) | 0.4.0 | MIT OR Apache-2.0 |
| [unicode-ident](https://github.com/dtolnay/unicode-ident) | 1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| [unicode-linebreak](https://github.com/axelf4/unicode-linebreak) | 0.1.5 | Apache-2.0 |
| [unicode-properties](https://github.com/unicode-rs/unicode-properties) | 0.1.4 | MIT OR Apache-2.0 |
| [unicode-script](https://github.com/unicode-rs/unicode-script) | 0.5.8 | MIT OR Apache-2.0 |
| [unicode-segmentation](https://github.com/unicode-rs/unicode-segmentation) | 1.13.3 | MIT OR Apache-2.0 |
| [unicode-vo](https://github.com/RazrFalcon/unicode-vo) | 0.1.0 | MIT OR Apache-2.0 |
| [unicode-width](https://github.com/unicode-rs/unicode-width) | 0.2.2 | MIT OR Apache-2.0 |
| [unicode-xid](https://github.com/unicode-rs/unicode-xid) | 0.2.6 | MIT OR Apache-2.0 |
| [url](https://github.com/servo/rust-url) | 2.5.8 | MIT OR Apache-2.0 |
| [usvg](https://github.com/linebender/resvg) | 0.46.0 | Apache-2.0 OR MIT |
| [utf8_iter](https://github.com/hsivonen/utf8_iter) | 1.0.4 | Apache-2.0 OR MIT |
| [uuid](https://github.com/uuid-rs/uuid) | 1.27.0 | Apache-2.0 OR MIT |
| [v_frame](https://github.com/rust-av/v_frame) | 0.3.9 | BSD-2-Clause |
| [value-bag](https://github.com/sval-rs/value-bag) | 1.14.1 | Apache-2.0 OR MIT |
| value-bag-serde1 | 1.14.1 | Apache-2.0 OR MIT |
| value-bag-sval2 | 1.14.1 | Apache-2.0 OR MIT |
| [waker-fn](https://github.com/smol-rs/waker-fn) | 1.2.0 | Apache-2.0 OR MIT |
| [walkdir](https://github.com/BurntSushi/walkdir) | 2.5.0 | Unlicense OR MIT |
| [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen) | 0.2.129 | MIT OR Apache-2.0 |
| [wasm-bindgen-macro](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro) | 0.2.129 | MIT OR Apache-2.0 |
| [wasm-bindgen-macro-support](https://github.com/wasm-bindgen/wasm-bindgen/tree/main/crates/macro-support) | 0.2.129 | MIT OR Apache-2.0 |
| [wasm-bindgen-shared](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/shared) | 0.2.129 | MIT OR Apache-2.0 |
| [wayland-backend](https://github.com/smithay/wayland-rs) | 0.3.17 | MIT |
| [wayland-client](https://github.com/smithay/wayland-rs) | 0.31.15 | MIT |
| [wayland-cursor](https://github.com/smithay/wayland-rs) | 0.31.14 | MIT |
| [wayland-protocols](https://github.com/smithay/wayland-rs) | 0.32.13 | MIT |
| [wayland-protocols-plasma](https://github.com/smithay/wayland-rs) | 0.3.12 | MIT |
| [wayland-protocols-wlr](https://github.com/smithay/wayland-rs) | 0.3.12 | MIT |
| [wayland-scanner](https://github.com/smithay/wayland-rs) | 0.31.11 | MIT |
| [wayland-sys](https://github.com/smithay/wayland-rs) | 0.31.11 | MIT |
| [web-time](https://github.com/daxpedda/web-time) | 1.1.0 | MIT OR Apache-2.0 |
| [weezl](https://github.com/image-rs/weezl) | 0.1.12 | MIT OR Apache-2.0 |
| [wgpu](https://github.com/gfx-rs/wgpu) | 29.0.4 | MIT OR Apache-2.0 |
| [wgpu-core](https://github.com/gfx-rs/wgpu) | 29.0.4 | MIT OR Apache-2.0 |
| [wgpu-core-deps-windows-linux-android](https://github.com/gfx-rs/wgpu) | 29.0.4 | MIT OR Apache-2.0 |
| [wgpu-hal](https://github.com/gfx-rs/wgpu) | 29.0.4 | MIT OR Apache-2.0 |
| [wgpu-naga-bridge](https://github.com/gfx-rs/wgpu) | 29.0.4 | MIT OR Apache-2.0 |
| [wgpu-types](https://github.com/gfx-rs/wgpu) | 29.0.4 | MIT OR Apache-2.0 |
| [which](https://github.com/harryfei/which-rs.git) | 8.0.6 | MIT |
| [winapi](https://github.com/retep998/winapi-rs) | 0.3.9 | MIT OR Apache-2.0 |
| [windows](https://github.com/microsoft/windows-rs) | 0.57.0 | MIT OR Apache-2.0 |
| [windows](https://github.com/microsoft/windows-rs) | 0.61.3 | MIT OR Apache-2.0 |
| [windows](https://github.com/microsoft/windows-rs) | 0.62.2 | MIT OR Apache-2.0 |
| [windows-capture](https://github.com/NiiightmareXD/windows-capture) | 1.5.0 | MIT |
| [windows-collections](https://github.com/microsoft/windows-rs) | 0.2.0 | MIT OR Apache-2.0 |
| [windows-collections](https://github.com/microsoft/windows-rs) | 0.3.2 | MIT OR Apache-2.0 |
| [windows-core](https://github.com/microsoft/windows-rs) | 0.57.0 | MIT OR Apache-2.0 |
| [windows-core](https://github.com/microsoft/windows-rs) | 0.61.2 | MIT OR Apache-2.0 |
| [windows-core](https://github.com/microsoft/windows-rs) | 0.62.2 | MIT OR Apache-2.0 |
| [windows-future](https://github.com/microsoft/windows-rs) | 0.2.1 | MIT OR Apache-2.0 |
| [windows-future](https://github.com/microsoft/windows-rs) | 0.3.2 | MIT OR Apache-2.0 |
| [windows-implement](https://github.com/microsoft/windows-rs) | 0.57.0 | MIT OR Apache-2.0 |
| [windows-implement](https://github.com/microsoft/windows-rs) | 0.60.2 | MIT OR Apache-2.0 |
| [windows-interface](https://github.com/microsoft/windows-rs) | 0.57.0 | MIT OR Apache-2.0 |
| [windows-interface](https://github.com/microsoft/windows-rs) | 0.59.3 | MIT OR Apache-2.0 |
| [windows-link](https://github.com/microsoft/windows-rs) | 0.1.3 | MIT OR Apache-2.0 |
| [windows-link](https://github.com/microsoft/windows-rs) | 0.2.1 | MIT OR Apache-2.0 |
| [windows-numerics](https://github.com/microsoft/windows-rs) | 0.2.0 | MIT OR Apache-2.0 |
| [windows-numerics](https://github.com/microsoft/windows-rs) | 0.3.1 | MIT OR Apache-2.0 |
| [windows-registry](https://github.com/microsoft/windows-rs) | 0.6.1 | MIT OR Apache-2.0 |
| [windows-result](https://github.com/microsoft/windows-rs) | 0.1.2 | MIT OR Apache-2.0 |
| [windows-result](https://github.com/microsoft/windows-rs) | 0.3.4 | MIT OR Apache-2.0 |
| [windows-result](https://github.com/microsoft/windows-rs) | 0.4.1 | MIT OR Apache-2.0 |
| [windows-strings](https://github.com/microsoft/windows-rs) | 0.4.2 | MIT OR Apache-2.0 |
| [windows-strings](https://github.com/microsoft/windows-rs) | 0.5.1 | MIT OR Apache-2.0 |
| [windows-sys](https://github.com/microsoft/windows-rs) | 0.61.2 | MIT OR Apache-2.0 |
| [windows-targets](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 |
| [windows-threading](https://github.com/microsoft/windows-rs) | 0.1.0 | MIT OR Apache-2.0 |
| [windows-threading](https://github.com/microsoft/windows-rs) | 0.2.1 | MIT OR Apache-2.0 |
| [windows_aarch64_msvc](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 |
| [windows_x86_64_msvc](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 |
| [winnow](https://github.com/winnow-rs/winnow) | 1.0.4 | MIT |
| [writeable](https://github.com/unicode-org/icu4x) | 0.6.4 | Unicode-3.0 |
| [x11](https://github.com/AltF02/x11-rs.git) | 2.21.0 | MIT |
| [x11-clipboard](https://github.com/quininer/x11-clipboard) | 0.9.3 | MIT |
| [x11rb](https://github.com/psychon/x11rb) | 0.13.2 | MIT OR Apache-2.0 |
| [x11rb-protocol](https://github.com/psychon/x11rb) | 0.13.2 | MIT OR Apache-2.0 |
| [xcb](https://github.com/rust-x-bindings/rust-xcb) | 1.7.1 | MIT |
| [xcursor](https://github.com/esposm03/xcursor-rs) | 0.3.11 | MIT |
| [xim-ctext](https://github.com/Riey/xim-rs) | 0.3.0 | MIT |
| [xim-parser](https://github.com/Riey/xim-rs) | 0.2.2 | MIT |
| [xkbcommon](https://github.com/rust-x-bindings/xkbcommon-rs) | 0.8.0 | MIT |
| [xkeysym](https://github.com/notgull/xkeysym) | 0.2.1 | MIT OR Apache-2.0 OR Zlib |
| [xmlwriter](https://github.com/RazrFalcon/xmlwriter) | 0.1.0 | MIT |
| [y4m](https://github.com/image-rs/y4m.git) | 0.8.0 | MIT |
| [yazi](https://github.com/dfrg/yazi) | 0.2.1 | Apache-2.0 OR MIT |
| [yeslogic-fontconfig-sys](https://github.com/yeslogic/fontconfig-rs) | 6.0.1 | MIT |
| [yoke](https://github.com/unicode-org/icu4x) | 0.8.3 | Unicode-3.0 |
| [yoke-derive](https://github.com/unicode-org/icu4x) | 0.8.4 | Unicode-3.0 |
| [zbus](https://github.com/z-galaxy/zbus/) | 5.19.0 | MIT |
| [zbus-lockstep](https://github.com/luukvanderduim/zbus-lockstep) | 0.5.2 | MIT |
| [zbus-lockstep-macros](https://github.com/luukvanderduim/zbus-lockstep) | 0.5.2 | MIT |
| [zbus_macros](https://github.com/z-galaxy/zbus/) | 5.19.0 | MIT |
| [zbus_names](https://github.com/z-galaxy/zbus/) | 4.3.4 | MIT |
| [zbus_xml](https://github.com/z-galaxy/zbus/) | 5.2.1 | MIT |
| [zcheapstr](https://github.com/z-galaxy/zcheapstr/) | 1.1.0 | MIT |
| [zed-font-kit](https://github.com/servo/font-kit) | 0.14.1-zed | MIT OR Apache-2.0 |
| [zed-scap](https://github.com/helmerapp/scap) | 0.0.8-zed | MIT |
| [zed-xim](https://github.com/Riey/xim-rs) | 0.4.0-zed | MIT |
| [zeno](https://github.com/dfrg/zeno) | 0.3.3 | Apache-2.0 OR MIT |
| [zerocopy](https://github.com/google/zerocopy) | 0.8.59 | BSD-2-Clause OR Apache-2.0 OR MIT |
| [zerocopy-derive](https://github.com/google/zerocopy) | 0.8.59 | BSD-2-Clause OR Apache-2.0 OR MIT |
| [zerofrom](https://github.com/unicode-org/icu4x) | 0.1.8 | Unicode-3.0 |
| [zerofrom-derive](https://github.com/unicode-org/icu4x) | 0.1.8 | Unicode-3.0 |
| [zeroize](https://github.com/RustCrypto/utils) | 1.9.0 | Apache-2.0 OR MIT |
| [zeroize_derive](https://github.com/RustCrypto/utils) | 1.5.0 | Apache-2.0 OR MIT |
| [zerotrie](https://github.com/unicode-org/icu4x) | 0.2.5 | Unicode-3.0 |
| [zerovec](https://github.com/unicode-org/icu4x) | 0.11.8 | Unicode-3.0 |
| [zerovec-derive](https://github.com/unicode-org/icu4x) | 0.11.6 | Unicode-3.0 |
| [zlib-rs](https://github.com/trifectatechfoundation/zlib-rs) | 0.6.8 | Zlib |
| [zmij](https://github.com/dtolnay/zmij) | 1.0.23 | MIT |
| [zune-core](https://github.com/etemesi254/zune-image) | 0.5.3 | MIT OR Apache-2.0 OR Zlib |
| zune-inflate | 0.2.54 | MIT OR Apache-2.0 OR Zlib |
| [zune-jpeg](https://github.com/etemesi254/zune-image/tree/dev/crates/zune-jpeg) | 0.5.15 | MIT OR Apache-2.0 OR Zlib |
| [zvariant](https://github.com/z-galaxy/zbus/) | 5.15.0 | MIT |
| [zvariant_derive](https://github.com/z-galaxy/zbus/) | 5.15.0 | MIT |
| [zvariant_utils](https://github.com/z-galaxy/zbus/) | 4.2.0 | MIT |
