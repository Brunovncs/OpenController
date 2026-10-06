# Third-party notices

Open Controller is MIT licensed (see [LICENSE](LICENSE)). Its programs include the code below,
each under its own license. ViGEmBus, HidHide, DsHidMini and BthPS3 are not included: Open
Controller downloads their official installers from Nefarius' GitHub releases when you ask it to.

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

## Rust crates

Each crate's license text is in its source, at the repository listed. Where a crate offers a
choice of licenses, Open Controller uses it under the first one that applies.

| Crate | Version | License |
|---|---|---|
| [accesskit](https://github.com/AccessKit/accesskit) | 0.24.1 | MIT OR Apache-2.0 |
| [accesskit_consumer](https://github.com/AccessKit/accesskit) | 0.38.0 | MIT OR Apache-2.0 |
| [accesskit_windows](https://github.com/AccessKit/accesskit) | 0.34.0 | MIT OR Apache-2.0 |
| [adler2](https://github.com/oyvindln/adler2) | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| [aho-corasick](https://github.com/BurntSushi/aho-corasick) | 1.1.5 | Unlicense OR MIT |
| [aligned](https://github.com/rust-embedded-community/aligned) | 0.4.3 | MIT OR Apache-2.0 |
| [aligned-vec](https://github.com/sarah-ek/aligned-vec/) | 0.6.4 | MIT |
| [allocator-api2](https://github.com/zakarumych/allocator-api2) | 0.2.21 | MIT OR Apache-2.0 |
| [anyhow](https://github.com/dtolnay/anyhow) | 1.0.104 | MIT OR Apache-2.0 |
| [arg_enum_proc_macro](https://github.com/lu-zero/arg_enum_proc_macro) | 0.3.4 | MIT |
| [arrayref](https://github.com/droundy/arrayref) | 0.3.9 | BSD-2-Clause |
| [arrayvec](https://github.com/bluss/arrayvec) | 0.7.8 | MIT OR Apache-2.0 |
| [as-slice](https://github.com/japaric/as-slice) | 0.2.1 | MIT OR Apache-2.0 |
| [async-channel](https://github.com/smol-rs/async-channel) | 2.5.0 | Apache-2.0 OR MIT |
| [async-compression](https://github.com/Nullus157/async-compression) | 0.4.50 | MIT OR Apache-2.0 |
| [async-task](https://github.com/smol-rs/async-task) | 4.7.1 | Apache-2.0 OR MIT |
| [atomic](https://github.com/Amanieu/atomic-rs) | 0.5.3 | Apache-2.0 OR MIT |
| [av-scenechange](https://github.com/rust-av/av-scenechange) | 0.14.1 | MIT |
| [av1-grain](https://github.com/rust-av/av1-grain) | 0.2.5 | BSD-2-Clause |
| [avif-serialize](https://github.com/kornelski/avif-serialize) | 0.8.9 | BSD-3-Clause |
| [backtrace](https://github.com/rust-lang/backtrace-rs) | 0.3.76 | MIT OR Apache-2.0 |
| [base64](https://github.com/marshallpierce/rust-base64) | 0.22.1 | MIT OR Apache-2.0 |
| [bit_field](https://github.com/phil-opp/rust-bit-field) | 0.10.3 | Apache-2.0 OR MIT |
| [bitflags](https://github.com/bitflags/bitflags) | 1.3.2 | MIT OR Apache-2.0 |
| [bitflags](https://github.com/bitflags/bitflags) | 2.13.2 | MIT OR Apache-2.0 |
| [bitstream-io](https://github.com/tuffy/bitstream-io) | 4.10.0 | MIT OR Apache-2.0 |
| [block-buffer](https://github.com/RustCrypto/utils) | 0.10.4 | MIT OR Apache-2.0 |
| [borsh](https://github.com/near/borsh-rs) | 1.8.1 | MIT OR Apache-2.0 |
| [bumpalo](https://github.com/fitzgen/bumpalo) | 3.20.3 | MIT OR Apache-2.0 |
| [bytemuck](https://github.com/Lokathor/bytemuck) | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| [bytemuck_derive](https://github.com/Lokathor/bytemuck) | 1.12.1 | Zlib OR Apache-2.0 OR MIT |
| [byteorder](https://github.com/BurntSushi/byteorder) | 1.5.0 | Unlicense OR MIT |
| [byteorder-lite](https://github.com/image-rs/byteorder-lite) | 0.1.0 | Unlicense OR MIT |
| [bytes](https://github.com/tokio-rs/bytes) | 1.12.1 | MIT |
| [bzip2](https://github.com/trifectatechfoundation/bzip2-rs) | 0.6.1 | MIT OR Apache-2.0 |
| [cfg-if](https://github.com/rust-lang/cfg-if) | 1.0.5 | MIT OR Apache-2.0 |
| [chrono](https://github.com/chronotope/chrono) | 0.4.45 | MIT OR Apache-2.0 |
| [color_quant](https://github.com/image-rs/color_quant.git) | 1.1.0 | MIT |
| [compression-codecs](https://github.com/Nullus157/async-compression) | 0.4.45 | MIT OR Apache-2.0 |
| [compression-core](https://github.com/Nullus157/async-compression) | 0.4.33 | MIT OR Apache-2.0 |
| [concurrent-queue](https://github.com/smol-rs/concurrent-queue) | 2.5.0 | Apache-2.0 OR MIT |
| [convert_case](https://github.com/rutrum/convert-case) | 0.10.0 | MIT |
| [core_maths](https://github.com/robertbastian/core_maths) | 0.1.1 | MIT |
| [cpufeatures](https://github.com/RustCrypto/utils) | 0.2.17 | MIT OR Apache-2.0 |
| [crc32fast](https://github.com/srijs/rust-crc32fast) | 1.5.2 | MIT OR Apache-2.0 |
| [crossbeam-channel](https://github.com/crossbeam-rs/crossbeam) | 0.5.17 | MIT OR Apache-2.0 |
| [crossbeam-deque](https://github.com/crossbeam-rs/crossbeam) | 0.8.8 | MIT OR Apache-2.0 |
| [crossbeam-epoch](https://github.com/crossbeam-rs/crossbeam) | 0.9.21 | MIT OR Apache-2.0 |
| [crossbeam-queue](https://github.com/crossbeam-rs/crossbeam) | 0.3.14 | MIT OR Apache-2.0 |
| [crossbeam-utils](https://github.com/crossbeam-rs/crossbeam) | 0.8.23 | MIT OR Apache-2.0 |
| [crypto-common](https://github.com/RustCrypto/traits) | 0.1.7 | MIT OR Apache-2.0 |
| [ctor](https://github.com/mmastrac/linktime) | 1.0.13 | Apache-2.0 OR MIT |
| [data-url](https://github.com/servo/rust-url) | 0.3.2 | MIT OR Apache-2.0 |
| [derive_more](https://github.com/JelteF/derive_more) | 2.1.1 | MIT |
| [derive_more-impl](https://github.com/JelteF/derive_more) | 2.1.1 | MIT |
| [digest](https://github.com/RustCrypto/traits) | 0.10.7 | MIT OR Apache-2.0 |
| [dirs](https://github.com/soc/dirs-rs) | 6.0.0 | MIT OR Apache-2.0 |
| [dirs-sys](https://github.com/dirs-dev/dirs-sys-rs) | 0.5.0 | MIT OR Apache-2.0 |
| [displaydoc](https://github.com/yaahc/displaydoc) | 0.2.7 | MIT OR Apache-2.0 |
| [dpi](https://github.com/rust-windowing/winit) | 0.1.2 | Apache-2.0 AND MIT |
| [dunce](https://gitlab.com/kornelski/dunce) | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| [dyn-clone](https://github.com/dtolnay/dyn-clone) | 1.0.20 | MIT OR Apache-2.0 |
| [either](https://github.com/rayon-rs/either) | 1.18.0 | MIT OR Apache-2.0 |
| [enumn](https://github.com/dtolnay/enumn) | 0.1.14 | MIT OR Apache-2.0 |
| [equator](https://github.com/sarah-ek/equator/) | 0.4.2 | MIT |
| [equator-macro](https://github.com/sarah-ek/equator/) | 0.4.2 | MIT |
| [equivalent](https://github.com/indexmap-rs/equivalent) | 1.0.2 | Apache-2.0 OR MIT |
| [erased-serde](https://github.com/dtolnay/erased-serde) | 0.4.10 | MIT OR Apache-2.0 |
| [etagere](https://github.com/nical/etagere) | 0.2.15 | MIT OR Apache-2.0 |
| [euclid](https://github.com/servo/euclid) | 0.22.14 | MIT OR Apache-2.0 |
| [event-listener](https://github.com/smol-rs/event-listener) | 5.4.2 | Apache-2.0 OR MIT |
| [event-listener-strategy](https://github.com/smol-rs/event-listener-strategy) | 0.5.4 | Apache-2.0 OR MIT |
| [exr](https://github.com/johannesvollmer/exrs) | 1.74.2 | BSD-3-Clause |
| [fastrand](https://github.com/smol-rs/fastrand) | 2.5.0 | Apache-2.0 OR MIT |
| [fax](https://github.com/pdf-rs/fax) | 0.2.7 | MIT |
| [fdeflate](https://github.com/image-rs/fdeflate) | 0.3.7 | MIT OR Apache-2.0 |
| [fixedbitset](https://github.com/petgraph/fixedbitset) | 0.5.7 | MIT OR Apache-2.0 |
| [flate2](https://github.com/rust-lang/flate2-rs) | 1.1.10 | MIT OR Apache-2.0 |
| [float-cmp](https://github.com/mikedilger/float-cmp) | 0.9.0 | MIT |
| [float_next_after](https://gitlab.com/bronsonbdevost/next_afterf) | 1.0.0 | MIT |
| [flume](https://github.com/zesterer/flume) | 0.12.0 | Apache-2.0 OR MIT |
| [foldhash](https://github.com/orlp/foldhash) | 0.2.0 | Zlib |
| [fontdb](https://github.com/RazrFalcon/fontdb) | 0.23.0 | MIT |
| [form_urlencoded](https://github.com/servo/rust-url) | 1.2.2 | MIT OR Apache-2.0 |
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
| [getrandom](https://github.com/rust-random/getrandom) | 0.3.4 | MIT OR Apache-2.0 |
| [getrandom](https://github.com/rust-random/getrandom) | 0.4.3 | MIT OR Apache-2.0 |
| [gif](https://github.com/image-rs/image-gif) | 0.14.2 | MIT OR Apache-2.0 |
| [gpui-pre](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-collections](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-derive-refineable](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-http-client](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-macros](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-perf](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-platform](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-refineable](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-scheduler](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-shared-string](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-sum-tree](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-util](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-util-macros](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-windows](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-zlog](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-ztracing](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [gpui-pre-ztracing-macro](https://github.com/zed-industries/zed) | 0.3.7 | Apache-2.0 |
| [half](https://github.com/VoidStarKat/half-rs) | 2.7.1 | MIT OR Apache-2.0 |
| [hash32](https://github.com/japaric/hash32) | 0.3.1 | MIT OR Apache-2.0 |
| [hashbrown](https://github.com/rust-lang/hashbrown) | 0.16.1 | MIT OR Apache-2.0 |
| [hashbrown](https://github.com/rust-lang/hashbrown) | 0.17.1 | MIT OR Apache-2.0 |
| [heapless](https://github.com/rust-embedded/heapless) | 0.9.3 | MIT OR Apache-2.0 |
| [heck](https://github.com/withoutboats/heck) | 0.5.0 | MIT OR Apache-2.0 |
| [http](https://github.com/hyperium/http) | 1.5.0 | MIT OR Apache-2.0 |
| [http-body](https://github.com/hyperium/http-body) | 1.1.0 | MIT |
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
| [inventory](https://github.com/dtolnay/inventory) | 0.3.24 | MIT OR Apache-2.0 |
| [itertools](https://github.com/rust-itertools/itertools) | 0.14.0 | MIT OR Apache-2.0 |
| [itoa](https://github.com/dtolnay/itoa) | 1.0.18 | MIT OR Apache-2.0 |
| [keyboard-types](https://github.com/rust-windowing/keyboard-types) | 0.8.3 | MIT OR Apache-2.0 |
| [kurbo](https://github.com/linebender/kurbo) | 0.13.1 | Apache-2.0 OR MIT |
| [lazy_static](https://github.com/rust-lang-nursery/lazy-static.rs) | 1.5.1 | MIT OR Apache-2.0 |
| [lebe](https://github.com/johannesvollmer/lebe) | 0.5.3 | BSD-3-Clause |
| [libbz2-rs-sys](https://github.com/trifectatechfoundation/libbzip2-rs) | 0.2.5 | bzip2-1.0.6 |
| [libc](https://github.com/rust-lang/libc) | 0.2.190 | MIT OR Apache-2.0 |
| [libm](https://github.com/rust-lang/compiler-builtins) | 0.2.16 | MIT |
| [link-section](https://github.com/mmastrac/linktime) | 0.19.3 | Apache-2.0 OR MIT |
| [linktime-proc-macro](https://github.com/mmastrac/linktime) | 0.2.3 | Apache-2.0 OR MIT |
| [litemap](https://github.com/unicode-org/icu4x) | 0.8.3 | Unicode-3.0 |
| [lock_api](https://github.com/Amanieu/parking_lot) | 0.4.14 | MIT OR Apache-2.0 |
| [log](https://github.com/rust-lang/log) | 0.4.34 | MIT OR Apache-2.0 |
| [loop9](https://gitlab.com/kornelski/loop9.git) | 0.1.5 | MIT |
| [lyon](https://github.com/nical/lyon) | 1.0.19 | MIT OR Apache-2.0 |
| [lyon_algorithms](https://github.com/nical/lyon) | 1.0.21 | MIT OR Apache-2.0 |
| [lyon_geom](https://github.com/nical/lyon) | 1.0.19 | MIT OR Apache-2.0 |
| [lyon_path](https://github.com/nical/lyon) | 1.0.19 | MIT OR Apache-2.0 |
| [lyon_tessellation](https://github.com/nical/lyon) | 1.0.22 | MIT OR Apache-2.0 |
| [maybe-rayon](https://github.com/shssoichiro/maybe-rayon) | 0.1.1 | MIT |
| [memchr](https://github.com/BurntSushi/memchr) | 2.8.3 | Unlicense OR MIT |
| [memmap2](https://github.com/RazrFalcon/memmap2-rs) | 0.9.11 | MIT OR Apache-2.0 |
| [miniz_oxide](https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide) | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| [miniz_oxide](https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide) | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| [moxcms](https://github.com/awxkee/moxcms.git) | 0.8.1 | BSD-3-Clause OR Apache-2.0 |
| [muda](https://github.com/tauri-apps/muda) | 0.21.0 | Apache-2.0 OR MIT |
| [new_debug_unreachable](https://github.com/mbrubeck/rust-debug-unreachable) | 1.0.6 | MIT |
| [no_std_io2](https://github.com/wcampbell0x2a/no-std-io2) | 0.9.4 | Apache-2.0 OR MIT |
| [nom](https://github.com/rust-bakery/nom) | 8.0.0 | MIT |
| [noop_proc_macro](https://github.com/lu-zero/noop_proc_macro) | 0.3.0 | MIT |
| [ntapi](https://github.com/MSxDOS/ntapi) | 0.4.3 | Apache-2.0 OR MIT |
| [nu-ansi-term](https://github.com/nushell/nu-ansi-term) | 0.50.3 | MIT |
| [num-bigint](https://github.com/rust-num/num-bigint) | 0.4.8 | MIT OR Apache-2.0 |
| [num-complex](https://github.com/rust-num/num-complex) | 0.4.6 | MIT OR Apache-2.0 |
| [num-derive](https://github.com/rust-num/num-derive) | 0.4.2 | MIT OR Apache-2.0 |
| [num-integer](https://github.com/rust-num/num-integer) | 0.1.47 | MIT OR Apache-2.0 |
| [num-rational](https://github.com/rust-num/num-rational) | 0.4.2 | MIT OR Apache-2.0 |
| [num-traits](https://github.com/rust-num/num-traits) | 0.2.19 | MIT OR Apache-2.0 |
| [num_cpus](https://github.com/seanmonstar/num_cpus) | 1.17.0 | MIT OR Apache-2.0 |
| [once_cell](https://github.com/matklad/once_cell) | 1.21.4 | MIT OR Apache-2.0 |
| [option-ext](https://github.com/soc/option-ext.git) | 0.2.0 | MPL-2.0 |
| [parking](https://github.com/smol-rs/parking) | 2.2.1 | Apache-2.0 OR MIT |
| [parking_lot](https://github.com/Amanieu/parking_lot) | 0.12.5 | MIT OR Apache-2.0 |
| [parking_lot_core](https://github.com/Amanieu/parking_lot) | 0.9.12 | MIT OR Apache-2.0 |
| [paste](https://github.com/dtolnay/paste) | 1.0.15 | MIT OR Apache-2.0 |
| [pastey](https://github.com/as1100k/pastey) | 0.1.1 | MIT OR Apache-2.0 |
| [percent-encoding](https://github.com/servo/rust-url/) | 2.3.2 | MIT OR Apache-2.0 |
| [pico-args](https://github.com/RazrFalcon/pico-args) | 0.5.0 | MIT |
| [pin-project](https://github.com/taiki-e/pin-project) | 1.1.13 | Apache-2.0 OR MIT |
| [pin-project-internal](https://github.com/taiki-e/pin-project) | 1.1.13 | Apache-2.0 OR MIT |
| [pin-project-lite](https://github.com/taiki-e/pin-project-lite) | 0.2.17 | Apache-2.0 OR MIT |
| [png](https://github.com/image-rs/image-png) | 0.17.16 | MIT OR Apache-2.0 |
| [png](https://github.com/image-rs/image-png) | 0.18.1 | MIT OR Apache-2.0 |
| [pollster](https://github.com/zesterer/pollster) | 0.2.5 | Apache-2.0 OR MIT |
| [pollster](https://github.com/zesterer/pollster) | 0.4.0 | Apache-2.0 OR MIT |
| [polycool](https://github.com/linebender/kurbo) | 0.4.0 | MIT OR Apache-2.0 |
| [postage](https://github.com/austinjones/postage-rs) | 0.5.0 | MIT |
| [potential_utf](https://github.com/unicode-org/icu4x) | 0.1.6 | Unicode-3.0 |
| [ppv-lite86](https://github.com/cryptocorrosion/cryptocorrosion) | 0.2.21 | MIT OR Apache-2.0 |
| [proc-macro-crate](https://github.com/bkchr/proc-macro-crate) | 3.5.0 | MIT OR Apache-2.0 |
| [proc-macro2](https://github.com/dtolnay/proc-macro2) | 1.0.107 | MIT OR Apache-2.0 |
| [profiling](https://github.com/aclysma/profiling) | 1.0.18 | MIT OR Apache-2.0 |
| [profiling-procmacros](https://github.com/aclysma/profiling) | 1.0.18 | MIT OR Apache-2.0 |
| [pulp](https://github.com/sarah-quinones/pulp/) | 0.22.3 | MIT |
| [pulp-wasm-simd-flag](https://github.com/sarah-quinones/pulp/) | 0.1.1 | MIT |
| [pxfm](https://github.com/awxkee/pxfm) | 0.1.30 | BSD-3-Clause OR Apache-2.0 |
| [qoi](https://github.com/aldanor/qoi-rust) | 0.4.1 | MIT OR Apache-2.0 |
| [quick-error](http://github.com/tailhook/quick-error) | 2.0.1 | MIT OR Apache-2.0 |
| [quote](https://github.com/dtolnay/quote) | 1.0.47 | MIT OR Apache-2.0 |
| [rand](https://github.com/rust-random/rand) | 0.9.5 | MIT OR Apache-2.0 |
| [rand_chacha](https://github.com/rust-random/rand) | 0.9.0 | MIT OR Apache-2.0 |
| [rand_core](https://github.com/rust-random/rand) | 0.9.5 | MIT OR Apache-2.0 |
| [rav1e](https://github.com/xiph/rav1e/) | 0.8.1 | BSD-2-Clause |
| [ravif](https://github.com/kornelski/cavif-rs) | 0.13.0 | BSD-3-Clause |
| [raw-cpuid](https://github.com/gz/rust-cpuid) | 11.6.0 | MIT |
| [raw-window-handle](https://github.com/rust-windowing/raw-window-handle) | 0.6.2 | MIT OR Apache-2.0 OR Zlib |
| [rayon](https://github.com/rayon-rs/rayon) | 1.12.0 | MIT OR Apache-2.0 |
| [rayon-core](https://github.com/rayon-rs/rayon) | 1.13.0 | MIT OR Apache-2.0 |
| [reborrow](https://github.com/sarah-ek/reborrow/) | 0.5.5 | MIT |
| [ref-cast](https://github.com/dtolnay/ref-cast) | 1.0.27 | MIT OR Apache-2.0 |
| [ref-cast-impl](https://github.com/dtolnay/ref-cast) | 1.0.27 | MIT OR Apache-2.0 |
| [regex](https://github.com/rust-lang/regex) | 1.13.1 | MIT OR Apache-2.0 |
| [regex-automata](https://github.com/rust-lang/regex) | 0.4.18 | MIT OR Apache-2.0 |
| [regex-syntax](https://github.com/rust-lang/regex) | 0.8.11 | MIT OR Apache-2.0 |
| [resvg](https://github.com/linebender/resvg) | 0.46.0 | Apache-2.0 OR MIT |
| [rgb](https://github.com/kornelski/rust-rgb) | 0.8.53 | MIT |
| [roxmltree](https://github.com/RazrFalcon/roxmltree) | 0.21.1 | MIT OR Apache-2.0 |
| [rustc-demangle](https://github.com/rust-lang/rustc-demangle) | 0.1.28 | MIT OR Apache-2.0 |
| [rustc-hash](https://github.com/rust-lang/rustc-hash) | 2.1.3 | Apache-2.0 OR MIT |
| [rustybuzz](https://github.com/harfbuzz/rustybuzz) | 0.20.1 | MIT |
| [ryu](https://github.com/dtolnay/ryu) | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| [schemars](https://github.com/GREsau/schemars) | 1.2.2 | MIT |
| [schemars_derive](https://github.com/GREsau/schemars) | 1.2.2 | MIT |
| [scopeguard](https://github.com/bluss/scopeguard) | 1.2.0 | MIT OR Apache-2.0 |
| [sdl3-sys](https://codeberg.org/maia/sdl3-sys-rs) | 0.7.2+SDL-3.4.18 | Zlib |
| [seahash](https://gitlab.redox-os.org/redox-os/seahash) | 4.1.0 | MIT |
| [serde](https://github.com/serde-rs/serde) | 1.0.229 | MIT OR Apache-2.0 |
| [serde_core](https://github.com/serde-rs/serde) | 1.0.229 | MIT OR Apache-2.0 |
| [serde_derive](https://github.com/serde-rs/serde) | 1.0.229 | MIT OR Apache-2.0 |
| [serde_derive_internals](https://github.com/serde-rs/serde) | 0.30.0 | MIT OR Apache-2.0 |
| [serde_fmt](https://github.com/KodrAus/serde_fmt.git) | 1.1.0 | Apache-2.0 OR MIT |
| [serde_json](https://github.com/serde-rs/json) | 1.0.151 | MIT OR Apache-2.0 |
| [serde_urlencoded](https://github.com/nox/serde_urlencoded) | 0.7.1 | MIT OR Apache-2.0 |
| [sha1_smol](https://github.com/mitsuhiko/sha1-smol) | 1.0.1 | BSD-3-Clause |
| [sha2](https://github.com/RustCrypto/hashes) | 0.10.9 | MIT OR Apache-2.0 |
| [sharded-slab](https://github.com/hawkw/sharded-slab) | 0.1.7 | MIT |
| [simd-adler32](https://github.com/mcountryman/simd-adler32) | 0.3.10 | MIT |
| [simd_helpers](https://github.com/lu-zero/simd_helpers) | 0.1.0 | MIT |
| [simplecss](https://github.com/linebender/simplecss) | 0.2.2 | Apache-2.0 OR MIT |
| [siphasher](https://github.com/jedisct1/rust-siphash) | 1.0.4 | MIT OR Apache-2.0 |
| [slab](https://github.com/tokio-rs/slab) | 0.4.12 | MIT |
| [slotmap](https://github.com/orlp/slotmap) | 1.1.1 | Zlib |
| [smallvec](https://github.com/servo/rust-smallvec) | 1.16.2 | MIT OR Apache-2.0 |
| [smol_str](https://github.com/rust-lang/rust-analyzer/tree/master/lib/smol_str) | 0.3.6 | MIT OR Apache-2.0 |
| [spin](https://github.com/mvdnes/spin-rs.git) | 0.10.1 | MIT |
| [spin](https://github.com/mvdnes/spin-rs.git) | 0.9.9 | MIT |
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
| [syn](https://github.com/dtolnay/syn) | 2.0.119 | MIT OR Apache-2.0 |
| [syn](https://github.com/dtolnay/syn) | 3.0.6 | MIT OR Apache-2.0 |
| [synstructure](https://github.com/mystor/synstructure) | 0.14.0 | MIT |
| [sysinfo](https://github.com/GuillaumeGomez/sysinfo) | 0.31.4 | MIT |
| [taffy](https://github.com/DioxusLabs/taffy) | 0.13.0 | MIT |
| [thiserror](https://github.com/dtolnay/thiserror) | 1.0.69 | MIT OR Apache-2.0 |
| [thiserror](https://github.com/dtolnay/thiserror) | 2.0.21 | MIT OR Apache-2.0 |
| [thiserror-impl](https://github.com/dtolnay/thiserror) | 1.0.69 | MIT OR Apache-2.0 |
| [thiserror-impl](https://github.com/dtolnay/thiserror) | 2.0.21 | MIT OR Apache-2.0 |
| [thread_local](https://github.com/Amanieu/thread_local-rs) | 1.1.10 | MIT OR Apache-2.0 |
| [tiff](https://github.com/image-rs/image-tiff) | 0.11.3 | MIT |
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
| [unicode-properties](https://github.com/unicode-rs/unicode-properties) | 0.1.4 | MIT OR Apache-2.0 |
| [unicode-script](https://github.com/unicode-rs/unicode-script) | 0.5.8 | MIT OR Apache-2.0 |
| [unicode-segmentation](https://github.com/unicode-rs/unicode-segmentation) | 1.13.3 | MIT OR Apache-2.0 |
| [unicode-vo](https://github.com/RazrFalcon/unicode-vo) | 0.1.0 | MIT OR Apache-2.0 |
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
| [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen) | 0.2.129 | MIT OR Apache-2.0 |
| [wasm-bindgen-macro](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/macro) | 0.2.129 | MIT OR Apache-2.0 |
| [wasm-bindgen-macro-support](https://github.com/wasm-bindgen/wasm-bindgen/tree/main/crates/macro-support) | 0.2.129 | MIT OR Apache-2.0 |
| [wasm-bindgen-shared](https://github.com/wasm-bindgen/wasm-bindgen/tree/master/crates/shared) | 0.2.129 | MIT OR Apache-2.0 |
| [web-time](https://github.com/daxpedda/web-time) | 1.1.0 | MIT OR Apache-2.0 |
| [weezl](https://github.com/image-rs/weezl) | 0.1.12 | MIT OR Apache-2.0 |
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
| [windows_x86_64_msvc](https://github.com/microsoft/windows-rs) | 0.52.6 | MIT OR Apache-2.0 |
| [winnow](https://github.com/winnow-rs/winnow) | 1.0.4 | MIT |
| [writeable](https://github.com/unicode-org/icu4x) | 0.6.4 | Unicode-3.0 |
| [xmlwriter](https://github.com/RazrFalcon/xmlwriter) | 0.1.0 | MIT |
| [y4m](https://github.com/image-rs/y4m.git) | 0.8.0 | MIT |
| [yoke](https://github.com/unicode-org/icu4x) | 0.8.3 | Unicode-3.0 |
| [yoke-derive](https://github.com/unicode-org/icu4x) | 0.8.4 | Unicode-3.0 |
| [zed-scap](https://github.com/helmerapp/scap) | 0.0.8-zed | MIT |
| [zerocopy](https://github.com/google/zerocopy) | 0.8.59 | BSD-2-Clause OR Apache-2.0 OR MIT |
| [zerocopy-derive](https://github.com/google/zerocopy) | 0.8.59 | BSD-2-Clause OR Apache-2.0 OR MIT |
| [zerofrom](https://github.com/unicode-org/icu4x) | 0.1.8 | Unicode-3.0 |
| [zerofrom-derive](https://github.com/unicode-org/icu4x) | 0.1.8 | Unicode-3.0 |
| [zerotrie](https://github.com/unicode-org/icu4x) | 0.2.5 | Unicode-3.0 |
| [zerovec](https://github.com/unicode-org/icu4x) | 0.11.8 | Unicode-3.0 |
| [zerovec-derive](https://github.com/unicode-org/icu4x) | 0.11.6 | Unicode-3.0 |
| [zlib-rs](https://github.com/trifectatechfoundation/zlib-rs) | 0.6.8 | Zlib |
| [zmij](https://github.com/dtolnay/zmij) | 1.0.23 | MIT |
| [zune-core](https://github.com/etemesi254/zune-image) | 0.5.3 | MIT OR Apache-2.0 OR Zlib |
| zune-inflate | 0.2.54 | MIT OR Apache-2.0 OR Zlib |
| [zune-jpeg](https://github.com/etemesi254/zune-image/tree/dev/crates/zune-jpeg) | 0.5.15 | MIT OR Apache-2.0 OR Zlib |
