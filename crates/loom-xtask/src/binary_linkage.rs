//! `glibc-floor <binary> --max <version>` and `static-binary <binary>`: what a released
//! `b10x-loom` asks of the loader on the machine it starts on
//! (`story:release-binaries-glibc-floor`).
//!
//! Both read the binary's ELF program headers and its dynamic section in Rust, with no `objdump`
//! or `readelf`: the program interpreter (`PT_INTERP`), the shared libraries it names
//! (`DT_NEEDED`) and the symbol versions it needs from them (`DT_VERNEED`, the table
//! `objdump -T` prints its versions from). Section headers are not read, so a stripped binary
//! reads the same. Only 64-bit little-endian ELF is read, the one kind a release builds; any other
//! file is refused by name.
//!
//! * `glibc-floor` fails when the binary needs a `GLIBC_x.y` version above `--max`, naming the
//!   highest and each one above, or when it needs no `GLIBC_` version at all (it is then not the
//!   glibc build the check was asked about). A weak version counts like any other.
//! * `static-binary` fails when the binary has a program interpreter or names a shared library.

use std::fs;
use std::path::Path;

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;
const DT_STRTAB: u64 = 5;
const DT_STRSZ: u64 = 10;
const DT_VERNEED: u64 = 0x6fff_fffe;
const DT_VERNEEDNUM: u64 = 0x6fff_ffff;
/// The size of an `Elf64_Phdr`, an `Elf64_Dyn`, an `Elf64_Verneed` and an `Elf64_Vernaux`.
const PHDR: u64 = 56;
const DYN: u64 = 16;
/// Every version needed from glibc starts with it.
const GLIBC: &str = "GLIBC_";

/// What a binary asks of the dynamic loader.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Linkage {
    /// The program interpreter (`PT_INTERP`), e.g. `/lib64/ld-linux-x86-64.so.2`.
    pub interpreter: Option<String>,
    /// The shared libraries it names (`DT_NEEDED`), in the file's order.
    pub needed: Vec<String>,
    /// Each symbol version it needs, as (library, version), in the file's order.
    pub versions: Vec<(String, String)>,
}

/// Bounds-checked little-endian reads over the file's bytes.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn slice(&self, at: u64, len: u64) -> Result<&[u8], String> {
        let end = at
            .checked_add(len)
            .ok_or_else(|| format!("offset {at:#x} + {len:#x} overflows"))?;
        usize::try_from(at)
            .ok()
            .zip(usize::try_from(end).ok())
            .and_then(|(at, end)| self.0.get(at..end))
            .ok_or_else(|| format!("{len} bytes at {at:#x} lie outside the file"))
    }

    fn u16(&self, at: u64) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.slice(at, 2)?.try_into().unwrap()))
    }

    fn u32(&self, at: u64) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.slice(at, 4)?.try_into().unwrap()))
    }

    fn u64(&self, at: u64) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.slice(at, 8)?.try_into().unwrap()))
    }
}

/// The text of `bytes` up to its first NUL, which must be there.
fn c_string(bytes: &[u8], what: &str) -> Result<String, String> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| format!("{what} has no terminating NUL"))?;
    Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
}

fn add(a: u64, b: u64) -> Result<u64, String> {
    a.checked_add(b)
        .ok_or_else(|| format!("offset {a:#x} + {b:#x} overflows"))
}

/// Reads what the ELF file `bytes` asks of the dynamic loader. A file without `PT_DYNAMIC` needs
/// no library and no version.
pub fn linkage(bytes: &[u8]) -> Result<Linkage, String> {
    if bytes.len() < 64 || bytes[..4] != *b"\x7fELF" {
        return Err("not an ELF file".to_owned());
    }
    if bytes[4] != 2 {
        return Err("not a 64-bit ELF file".to_owned());
    }
    if bytes[5] != 1 {
        return Err("not a little-endian ELF file".to_owned());
    }
    let file = Reader(bytes);
    let phoff = file.u64(0x20)?;
    let phentsize = u64::from(file.u16(0x36)?);
    let phnum = u64::from(file.u16(0x38)?);
    if phnum > 0 && phentsize < PHDR {
        return Err(format!("program header entries of {phentsize} bytes"));
    }

    let mut linkage = Linkage::default();
    let mut loads = Vec::new();
    let mut dynamic = None;
    for index in 0..phnum {
        let at = add(phoff, index * phentsize)?;
        let offset = file.u64(add(at, 8)?)?;
        let vaddr = file.u64(add(at, 16)?)?;
        let filesz = file.u64(add(at, 32)?)?;
        match file.u32(at)? {
            PT_LOAD => loads.push((vaddr, offset, filesz)),
            PT_INTERP => {
                linkage.interpreter = Some(c_string(
                    file.slice(offset, filesz)?,
                    "the program interpreter",
                )?);
            }
            PT_DYNAMIC => dynamic = Some((offset, filesz)),
            _ => {}
        }
    }
    let Some((dynamic, size)) = dynamic else {
        return Ok(linkage);
    };

    let mut needed = Vec::new();
    let (mut strtab, mut strsz, mut verneed, mut verneednum) = (None, None, None, 0);
    for index in 0..size / DYN {
        let at = add(dynamic, index * DYN)?;
        let value = file.u64(add(at, 8)?)?;
        match file.u64(at)? {
            DT_NULL => break,
            DT_NEEDED => needed.push(value),
            DT_STRTAB => strtab = Some(value),
            DT_STRSZ => strsz = Some(value),
            DT_VERNEED => verneed = Some(value),
            DT_VERNEEDNUM => verneednum = value,
            _ => {}
        }
    }
    if needed.is_empty() && verneed.is_none() {
        return Ok(linkage);
    }
    let (Some(strtab), Some(strsz)) = (strtab, strsz) else {
        return Err("the dynamic section names no string table (DT_STRTAB, DT_STRSZ)".to_owned());
    };
    let strings = file.slice(file_offset(&loads, strtab)?, strsz)?;
    let string = |offset: u64| -> Result<String, String> {
        let tail = usize::try_from(offset)
            .ok()
            .and_then(|offset| strings.get(offset..))
            .ok_or_else(|| format!("string {offset:#x} lies outside the string table"))?;
        c_string(tail, &format!("string {offset:#x}"))
    };
    for offset in needed {
        linkage.needed.push(string(offset)?);
    }

    if let Some(verneed) = verneed {
        let mut at = file_offset(&loads, verneed)?;
        for _ in 0..verneednum {
            let count = file.u16(add(at, 2)?)?;
            let library = string(u64::from(file.u32(add(at, 4)?)?))?;
            let mut aux = add(at, u64::from(file.u32(add(at, 8)?)?))?;
            for _ in 0..count {
                linkage
                    .versions
                    .push((library.clone(), string(u64::from(file.u32(add(aux, 8)?)?))?));
                match file.u32(add(aux, 12)?)? {
                    0 => break,
                    next => aux = add(aux, u64::from(next))?,
                }
            }
            match file.u32(add(at, 12)?)? {
                0 => break,
                next => at = add(at, u64::from(next))?,
            }
        }
    }
    Ok(linkage)
}

/// The file offset of the virtual address `address`, through the `PT_LOAD` segment holding it.
fn file_offset(loads: &[(u64, u64, u64)], address: u64) -> Result<u64, String> {
    loads
        .iter()
        .find(|(vaddr, _, filesz)| *vaddr <= address && address - vaddr < *filesz)
        .ok_or_else(|| format!("address {address:#x} lies in no loaded segment"))
        .and_then(|(vaddr, offset, _)| add(*offset, address - vaddr))
}

/// A glibc version, `2.17` or `2.2.5`, compared part by part as numbers.
fn version(text: &str) -> Option<Vec<u32>> {
    text.split('.')
        .map(|part| part.parse().ok())
        .collect::<Option<Vec<u32>>>()
        .filter(|parts| parts.len() >= 2)
}

/// Every `GLIBC_` version `linkage` needs, once each, lowest first.
fn glibc_versions(linkage: &Linkage) -> Result<Vec<(Vec<u32>, String)>, String> {
    let mut versions = Vec::new();
    for (library, name) in &linkage.versions {
        let Some(number) = name.strip_prefix(GLIBC) else {
            continue;
        };
        let parsed = version(number).ok_or_else(|| {
            format!("{library} is needed at {name}, which is no numbered glibc version")
        })?;
        versions.push((parsed, name.clone()));
    }
    versions.sort();
    versions.dedup();
    Ok(versions)
}

/// The verdict on `linkage` against the glibc floor `max`: the line to print, or the error.
fn floor_verdict(binary: &Path, linkage: &Linkage, max: &str) -> Result<String, String> {
    let floor = version(max).ok_or_else(|| format!("`{max}` is not a glibc version (2.17)"))?;
    let versions = glibc_versions(linkage)?;
    let Some((_, highest)) = versions.last() else {
        return Err(format!(
            "`{}` needs no {GLIBC} symbol version: it is not linked against glibc",
            binary.display()
        ));
    };
    let above: Vec<&str> = versions
        .iter()
        .filter(|(parts, _)| *parts > floor)
        .map(|(_, name)| name.as_str())
        .collect();
    if above.is_empty() {
        Ok(format!(
            "`{}` needs {highest} at most, within the glibc floor {max}",
            binary.display()
        ))
    } else {
        Err(format!(
            "`{}` needs {highest}, above the glibc floor {max}: {} (`objdump -T` names the symbols)",
            binary.display(),
            above.join(", ")
        ))
    }
}

/// The verdict on `linkage` for a binary that must start without a dynamic loader.
fn static_verdict(binary: &Path, linkage: &Linkage) -> Result<String, String> {
    let mut found = Vec::new();
    if let Some(interpreter) = &linkage.interpreter {
        found.push(format!("the program interpreter {interpreter}"));
    }
    for library in &linkage.needed {
        found.push(format!("the shared library {library}"));
    }
    if found.is_empty() {
        Ok(format!(
            "`{}` is static: no program interpreter and no shared library",
            binary.display()
        ))
    } else {
        Err(format!(
            "`{}` is not static: it names {}",
            binary.display(),
            found.join(", ")
        ))
    }
}

fn read(binary: &Path) -> Result<Linkage, String> {
    let bytes = fs::read(binary).map_err(|e| format!("read `{}`: {e}", binary.display()))?;
    linkage(&bytes).map_err(|e| format!("`{}`: {e}", binary.display()))
}

/// Fails when `binary` needs a `GLIBC_` version above `max`, naming the highest.
pub fn glibc_floor(binary: &Path, max: &str) -> Result<(), String> {
    println!("{}", floor_verdict(binary, &read(binary)?, max)?);
    Ok(())
}

/// Fails when `binary` has a program interpreter or names a shared library.
pub fn static_binary(binary: &Path) -> Result<(), String> {
    println!("{}", static_verdict(binary, &read(binary)?)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The address the fixtures load at, so an address must be translated to reach its bytes.
    const BASE: u64 = 0x40_0000;

    /// A 64-bit little-endian ELF file with one `PT_LOAD` over the whole file at [`BASE`], an
    /// optional `PT_INTERP`, and, when `dynamic`, a `PT_DYNAMIC` naming `needed` and `versions`.
    fn fixture(
        interpreter: Option<&str>,
        dynamic: bool,
        needed: &[&str],
        versions: &[(&str, &[&str])],
    ) -> Vec<u8> {
        let phnum = 1 + u64::from(interpreter.is_some()) + u64::from(dynamic);
        let start = 64 + PHDR * phnum;
        let mut body: Vec<u8> = Vec::new();
        let at = |body: &Vec<u8>| start + body.len() as u64;

        let interp = interpreter.map(|path| {
            let offset = at(&body);
            body.extend_from_slice(path.as_bytes());
            body.push(0);
            (offset, path.len() as u64 + 1)
        });

        let mut strings = vec![0u8];
        let mut intern = |text: &str| {
            let offset = strings.len() as u32;
            strings.extend_from_slice(text.as_bytes());
            strings.push(0);
            offset
        };
        let needed: Vec<u32> = needed.iter().map(|name| intern(name)).collect();
        let versions: Vec<(u32, Vec<u32>)> = versions
            .iter()
            .map(|(library, names)| (intern(library), names.iter().map(|n| intern(n)).collect()))
            .collect();
        let strtab = at(&body);
        let strsz = strings.len() as u64;
        body.extend_from_slice(&strings);

        let verneed = at(&body);
        for (index, (library, names)) in versions.iter().enumerate() {
            let size = DYN * (1 + names.len() as u64);
            body.extend_from_slice(&1u16.to_le_bytes());
            body.extend_from_slice(&(names.len() as u16).to_le_bytes());
            body.extend_from_slice(&library.to_le_bytes());
            body.extend_from_slice(&(DYN as u32).to_le_bytes());
            let next = if index + 1 < versions.len() { size } else { 0 };
            body.extend_from_slice(&(next as u32).to_le_bytes());
            for (position, name) in names.iter().enumerate() {
                body.extend_from_slice(&0u32.to_le_bytes());
                body.extend_from_slice(&0u16.to_le_bytes());
                body.extend_from_slice(&2u16.to_le_bytes());
                body.extend_from_slice(&name.to_le_bytes());
                let next = if position + 1 < names.len() { DYN } else { 0 };
                body.extend_from_slice(&(next as u32).to_le_bytes());
            }
        }

        let dynamic_at = at(&body);
        let mut entries = Vec::new();
        for offset in &needed {
            entries.push((DT_NEEDED, u64::from(*offset)));
        }
        entries.push((DT_STRTAB, BASE + strtab));
        entries.push((DT_STRSZ, strsz));
        if !versions.is_empty() {
            entries.push((DT_VERNEED, BASE + verneed));
            entries.push((DT_VERNEEDNUM, versions.len() as u64));
        }
        entries.push((DT_NULL, 0));
        if dynamic {
            for (tag, value) in &entries {
                body.extend_from_slice(&tag.to_le_bytes());
                body.extend_from_slice(&value.to_le_bytes());
            }
        }
        let total = at(&body);

        let mut file = Vec::new();
        file.extend_from_slice(b"\x7fELF\x02\x01\x01");
        file.resize(16, 0);
        file.extend_from_slice(&3u16.to_le_bytes()); // ET_DYN
        file.extend_from_slice(&62u16.to_le_bytes()); // EM_X86_64
        file.extend_from_slice(&1u32.to_le_bytes());
        file.extend_from_slice(&0u64.to_le_bytes()); // e_entry
        file.extend_from_slice(&64u64.to_le_bytes()); // e_phoff
        file.extend_from_slice(&0u64.to_le_bytes()); // e_shoff
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&64u16.to_le_bytes());
        file.extend_from_slice(&(PHDR as u16).to_le_bytes());
        file.extend_from_slice(&(phnum as u16).to_le_bytes());
        file.extend_from_slice(&64u16.to_le_bytes());
        file.extend_from_slice(&0u16.to_le_bytes());
        file.extend_from_slice(&0u16.to_le_bytes());
        let mut phdr = |kind: u32, offset: u64, size: u64| {
            file.extend_from_slice(&kind.to_le_bytes());
            file.extend_from_slice(&4u32.to_le_bytes());
            file.extend_from_slice(&offset.to_le_bytes());
            file.extend_from_slice(&(BASE + offset).to_le_bytes());
            file.extend_from_slice(&(BASE + offset).to_le_bytes());
            file.extend_from_slice(&size.to_le_bytes());
            file.extend_from_slice(&size.to_le_bytes());
            file.extend_from_slice(&8u64.to_le_bytes());
        };
        phdr(PT_LOAD, 0, total);
        if let Some((offset, size)) = interp {
            phdr(PT_INTERP, offset, size);
        }
        if dynamic {
            phdr(PT_DYNAMIC, dynamic_at, DYN * entries.len() as u64);
        }
        assert_eq!(file.len() as u64, start);
        file.extend_from_slice(&body);
        file
    }

    const LOADER: &str = "/lib64/ld-linux-x86-64.so.2";

    /// A glibc build as `objdump -T` would show it: versions from libgcc_s, libm and libc.
    fn glibc_build(libc: &[&str]) -> Vec<u8> {
        fixture(
            Some(LOADER),
            true,
            &["libgcc_s.so.1", "libm.so.6", "libc.so.6"],
            &[
                ("libgcc_s.so.1", &["GCC_3.0", "GCC_4.2.0"]),
                ("libm.so.6", &["GLIBC_2.2.5"]),
                ("libc.so.6", libc),
            ],
        )
    }

    fn floor(bytes: &[u8], max: &str) -> Result<String, String> {
        floor_verdict(Path::new("b10x-loom"), &linkage(bytes)?, max)
    }

    #[test]
    fn linkage_reads_the_interpreter_the_libraries_and_the_versions() {
        let read = linkage(&glibc_build(&["GLIBC_2.2.5", "GLIBC_2.17"])).unwrap();
        assert_eq!(read.interpreter.as_deref(), Some(LOADER));
        assert_eq!(read.needed, ["libgcc_s.so.1", "libm.so.6", "libc.so.6"]);
        let versions: Vec<(&str, &str)> = read
            .versions
            .iter()
            .map(|(library, name)| (library.as_str(), name.as_str()))
            .collect();
        assert_eq!(
            versions,
            [
                ("libgcc_s.so.1", "GCC_3.0"),
                ("libgcc_s.so.1", "GCC_4.2.0"),
                ("libm.so.6", "GLIBC_2.2.5"),
                ("libc.so.6", "GLIBC_2.2.5"),
                ("libc.so.6", "GLIBC_2.17"),
            ]
        );
    }

    #[test]
    fn glibc_floor_passes_a_binary_at_the_floor() {
        let said = floor(
            &glibc_build(&["GLIBC_2.2.5", "GLIBC_2.3", "GLIBC_2.17"]),
            "2.17",
        )
        .expect("a binary that needs GLIBC_2.17 at most passes the floor 2.17");
        assert_eq!(
            said,
            "`b10x-loom` needs GLIBC_2.17 at most, within the glibc floor 2.17"
        );
    }

    /// The versions of the `0.61.0` release binary built on Debian bookworm (`objdump -T`).
    #[test]
    fn glibc_floor_refuses_a_binary_above_the_floor_and_names_the_highest() {
        let bookworm = glibc_build(&[
            "GLIBC_2.2.5",
            "GLIBC_2.17",
            "GLIBC_2.18",
            "GLIBC_2.28",
            "GLIBC_2.34",
            "GLIBC_2.33",
            "GLIBC_2.3.4",
        ]);
        let error = floor(&bookworm, "2.17").expect_err("GLIBC_2.34 is above 2.17");
        assert_eq!(
            error,
            "`b10x-loom` needs GLIBC_2.34, above the glibc floor 2.17: GLIBC_2.18, GLIBC_2.28, \
             GLIBC_2.33, GLIBC_2.34 (`objdump -T` names the symbols)"
        );
        floor(&bookworm, "2.34").expect("the same binary passes the floor 2.34");
    }

    /// `2.3` is below `2.10` and `2.2.5` below `2.17`: versions compare as numbers, not text.
    #[test]
    fn glibc_versions_compare_part_by_part_as_numbers() {
        let read = glibc_build(&["GLIBC_2.10", "GLIBC_2.3", "GLIBC_2.2.5", "GLIBC_2.9"]);
        assert_eq!(
            floor(&read, "2.9").expect_err("2.10 is above 2.9"),
            "`b10x-loom` needs GLIBC_2.10, above the glibc floor 2.9: GLIBC_2.10 \
             (`objdump -T` names the symbols)"
        );
        floor(&read, "2.10").expect("2.10 is the highest");
    }

    #[test]
    fn glibc_floor_refuses_a_binary_without_glibc_and_a_version_it_cannot_order() {
        let musl = fixture(None, true, &[], &[]);
        assert_eq!(
            floor(&musl, "2.17").expect_err("a static binary needs no glibc"),
            "`b10x-loom` needs no GLIBC_ symbol version: it is not linked against glibc"
        );
        let private = glibc_build(&["GLIBC_2.2.5", "GLIBC_PRIVATE"]);
        let error = floor(&private, "2.17").expect_err("GLIBC_PRIVATE has no number");
        assert!(error.contains("GLIBC_PRIVATE"), "{error}");
        let error = floor(&glibc_build(&["GLIBC_2.2.5"]), "2").expect_err("2 is no version");
        assert!(error.contains("`2` is not a glibc version"), "{error}");
    }

    #[test]
    fn static_binary_refuses_an_interpreter_or_a_shared_library() {
        let error = static_verdict(
            Path::new("b10x-loom"),
            &linkage(&glibc_build(&["GLIBC_2.2.5"])).unwrap(),
        )
        .expect_err("a glibc build is not static");
        assert_eq!(
            error,
            "`b10x-loom` is not static: it names the program interpreter \
             /lib64/ld-linux-x86-64.so.2, the shared library libgcc_s.so.1, the shared library \
             libm.so.6, the shared library libc.so.6"
        );
        let no_interpreter = fixture(None, true, &["libc.musl-x86_64.so.1"], &[]);
        let error = static_verdict(Path::new("b"), &linkage(&no_interpreter).unwrap())
            .expect_err("a needed library without an interpreter is not static");
        assert!(
            error.ends_with("the shared library libc.musl-x86_64.so.1"),
            "{error}"
        );
        let interpreter_only = fixture(Some("/lib/ld-musl-x86_64.so.1"), false, &[], &[]);
        assert!(static_verdict(Path::new("b"), &linkage(&interpreter_only).unwrap()).is_err());
    }

    /// A static-pie musl build keeps `PT_DYNAMIC` for its relocations but names no library; a
    /// plain static build has no dynamic section at all. Both are static.
    #[test]
    fn static_binary_passes_static_and_static_pie_builds() {
        for bytes in [
            fixture(None, true, &[], &[]),
            fixture(None, false, &[], &[]),
        ] {
            let said = static_verdict(Path::new("b10x-loom"), &linkage(&bytes).unwrap())
                .expect("a binary without interpreter and libraries is static");
            assert_eq!(
                said,
                "`b10x-loom` is static: no program interpreter and no shared library"
            );
        }
    }

    /// Every prefix of a fixture, and a few corrupted headers, read to an answer without a panic.
    #[test]
    fn a_truncated_or_foreign_file_is_refused_without_a_panic() {
        let whole = glibc_build(&["GLIBC_2.2.5", "GLIBC_2.17"]);
        for end in 0..whole.len() {
            let _ = linkage(&whole[..end]);
        }
        assert_eq!(linkage(b"#!/bin/sh\n").unwrap_err(), "not an ELF file");
        let mut thirty_two = whole.clone();
        thirty_two[4] = 1;
        assert_eq!(linkage(&thirty_two).unwrap_err(), "not a 64-bit ELF file");
        let mut big_endian = whole.clone();
        big_endian[5] = 2;
        assert_eq!(
            linkage(&big_endian).unwrap_err(),
            "not a little-endian ELF file"
        );
        let mut far = whole;
        far[0x20..0x28].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(linkage(&far).is_err());
    }

    /// The test binary itself, as the host's linker made it: glibc's loader, `libc.so.6`, and the
    /// x86-64 baseline version `GLIBC_2.2.5`.
    #[cfg(all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]
    #[test]
    fn linkage_reads_a_binary_the_host_linked() {
        let path = std::env::current_exe().unwrap();
        let read = linkage(&fs::read(&path).unwrap()).unwrap();
        assert!(
            read.interpreter
                .as_deref()
                .is_some_and(|loader| loader.ends_with("ld-linux-x86-64.so.2")),
            "{read:?}"
        );
        assert!(
            read.needed.iter().any(|library| library == "libc.so.6"),
            "{read:?}"
        );
        assert!(
            read.versions
                .iter()
                .any(|(library, name)| library == "libc.so.6" && name == "GLIBC_2.2.5"),
            "{read:?}"
        );
        floor_verdict(&path, &read, "999.0").expect("no glibc version reaches 999.0");
    }
}
