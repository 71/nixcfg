extern crate rnix;
extern crate structopt;

use std::fmt::Write as FmtWrite;
use std::io::{stdin, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use rnix::tokenizer::TokenKind;
use rnix::parser::{AST, Data as ASTData, ASTKind, ASTNode, NodeId};

use structopt::{clap::AppSettings, StructOpt};


#[derive(Debug, StructOpt)]
#[structopt(
    raw(global_settings = "&[AppSettings::DisableHelpSubcommand,
                             AppSettings::InferSubcommands,
                             AppSettings::VersionlessSubcommands]")
)]
pub struct Args {
    /// Input .nix file to query or modify.
    #[structopt(short = "f", long = "file", default_value = "/etc/nixos/configuration.nix", parse(from_os_str))]
    input: PathBuf,

    /// Modify in place instead of printing result to stdout.
    #[structopt(short = "i", long = "in-place")]
    in_place: bool,

    /// Command to execute.
    #[structopt(subcommand)]
    command: Command
}

#[derive(Debug, StructOpt)]
pub enum Command {
    /// Get the value at the given path.
    #[structopt(name = "get")]
    Get {
        /// The path of the value.
        #[structopt(name = "path")]
        path: String
    },

    /// Set the value at the given path.
    #[structopt(name = "set")]
    Set {
        /// The path of the value.
        #[structopt(name = "path")]
        path: String,

        /// The new value. If not specified, it will be read from stdin.
        #[structopt(name = "value")]
        value: Option<String>,

        /// Do not strip last new-line character from input.
        #[structopt(short = "n", long = "keep-eol")]
        keep_eol: bool
    }
}

fn main() {
    let args = Args::from_args();

    if let Err(err) = run(args) {
        eprintln!("{}", err);
        std::process::exit(1)
    }
}


fn run(args: Args) -> Result<(), String> {
    let Args { in_place, input, command } = args;
    
    // Read file contents
    let mut file = std::fs::File::open(&input)
        .map_err(|err| format!("Unable to open file '{}': {}.", input.display(), err))?;
    
    let mut content = String::new();

    file.read_to_string(&mut content)
        .map_err(|err| format!("Unable to read input file: {}.", err))?;
    
    // Parse file contents
    let ast = rnix::parse(&content)
        .map_err(|err| format!("Unable to parse input file: {}.", err))?;

    // Process file
    process(ast, command, &mut content)?;

    // Write output
    if in_place {
        file.seek(SeekFrom::Start(0))
            .map_err(|err| format!("Unable to seek to the start of the input file: {}.", err))?;
        
        write!(file, "{}", content);

        // Resize file if needed
        let file_metadata = file.metadata()
                                .map_err(|err| format!("Unable to get output file metadata: {}.", err))?;

        if content.len() < file_metadata.len() as _ {
            file.set_len(content.len() as _)
                .map_err(|err| format!("Unable to resize input file after writing in-place: {}.", err))?;
        }
    } else {
        println!("{}", content);
    }

    // Everything worked, return
    Ok(())
}

fn process(mut ast: AST, command: Command, content: &mut String) -> Result<(), String> {
    let root = &ast.arena[ast.root];

    match command {
        Command::Get { path } => {
            let parts: Vec<_> = path.split('.').collect();
            let node = find_node(&ast, root, &parts, 0)?;

            // Since we individually display nodes, we have to set the
            // matching node as root of the AST and then display it whole
            ast.root = node;

            content.clear();
            write!(content, "{}", ast);

            // Trim output, since we may have some garbage
            let trunc = content.as_bytes()
                               .iter()
                               .rev()
                               .take_while(|ch| **ch == b' ' || **ch == b'\n')
                               .count();
            
            if trunc > 0 {
                let new_len = content.len() - trunc;

                content.truncate(new_len)
            }
        },

        Command::Set { path, value, keep_eol } => {
            let value = match value {
                Some(value) => value,
                None => {
                    let mut input = String::new();

                    stdin().read_to_string(&mut input)
                           .map_err(|err| format!("Could not read replacement value from stdin: {}.", err))?;
                    
                    let input_len = input.len();
                    
                    if !keep_eol && input.ends_with('\n') {
                        let new_len = input_len - (if input.ends_with("\r\n") { 2 } else { 1 });

                        input.truncate(new_len);
                    }
                    
                    input
                }
            };
            let parts: Vec<_> = path.split('.').collect();

            match find_node(&ast, root, &parts, 0) {
                Ok(node) => {
                    // We found a match, and we have to replace it
                    let node = &ast.arena[node];
                    let range = node.span.start as usize .. node.span.end.unwrap() as usize;
                    
                    content.replace_range(range, &value);
                },

                Err(_) => {
                    // We did not find a match, so we'll try to add the value ourselves
                    panic!("not implemented")
                }
            }
        }
    }

    Ok(())
}

fn find_node(ast: &AST, node: &ASTNode, parts: &[&str], i: usize) -> Result<NodeId, String> {
    let part = parts[i];

    /// Try to match the i'th child with the given path. On success, the ID of the r'th child
    /// will be returned.
    macro_rules! try_match {
        ( $i: expr => $r: expr ) => ({
            let ident_node = &ast.arena[node.children(&ast.arena).nth($i).unwrap()];
            let j = try_advance_ident(ast, ident_node, parts, i);

            if j > i {
                // We did advance, which means we might have a match
                let res_id = node.children(&ast.arena).nth($r).unwrap();

                if j == parts.len() {
                    // We even got to the end of the path, which means we have a complete match!
                    Ok(res_id)
                } else {
                    // We're not at the end of the path, so we continue recursively
                    find_node(ast, &ast.arena[res_id], parts, j)
                }
            } else {
                node.children(&ast.arena)
                    .filter_map(|id| find_node(ast, &ast.arena[id], parts, i).ok())
                    .nth(0)
                    .ok_or_else(|| format!("Part '{}' of path not found.", part))
            }
        });
    }

    match node.kind {
        ASTKind::Apply => try_match!(0 => 1),
        ASTKind::SetEntry => try_match!(0 => 2),

        // Try recursively on children
        _ => node.children(&ast.arena)
                 .filter_map(|id| find_node(ast, &ast.arena[id], parts, i).ok())
                 .nth(0)
                 .ok_or_else(|| format!("Part '{}' of path not found.", part))
    }
}

fn try_advance_ident(ast: &AST, node: &ASTNode, parts: &[&str], i: usize) -> usize {
    match node.kind {
        ASTKind::Attribute | ASTKind::IndexSet => {
            let mut j = i;

            for sub_node in node.children(&ast.arena) {
                let part = match parts.get(j) {
                    Some(part) => part,
                    None => return i
                };

                match &ast.arena[sub_node].data {
                    &ASTData::Ident(_, ref ident) => if ident == part {
                        j += 1
                    },
                    &ASTData::Token(_, TokenKind::Dot) => (),

                    data => panic!("Unexpected data: {:?}", data)
                }
            }

            j
        }

        // Try recursively on children
        _ => node.children(&ast.arena)
                 .map(|id| try_advance_ident(ast, &ast.arena[id], parts, i))
                 .find(|j| *j > i)
                 .unwrap_or(i)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::path::PathBuf;

    fn assert_value_eq(content: &str, path: &str, expected: &str) {
        let mut output = content.to_string();

        let result = process(rnix::parse(content).unwrap(),
                             Command::Get { path: path.to_string() }, &mut output);
        
        assert_eq!(result.map(|_| output.as_str()), Ok(expected))
    }

    #[test]
    fn test_simple_paths() {
        let nix = r#"
          let 
            nixpkgs-mozilla = fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz;
          in

          with import <nixpkgs> {
            overlays = [ (import nixpkgs-mozilla) ];
          };

          stdenv.mkDerivation { name = "foo"; buildInputs = [ latest.rustChannels.nightly.rust ]; }
        "#;

        assert_value_eq(nix, "nixpkgs-mozilla", "fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz");
        assert_value_eq(nix, "overlays", "[ (import nixpkgs-mozilla) ]");
        assert_value_eq(nix, "stdenv.mkDerivation", r#"{ name = "foo"; buildInputs = [ latest.rustChannels.nightly.rust ]; }"#);
        assert_value_eq(nix, "stdenv.mkDerivation.name", "\"foo\"");
    }

    #[test]
    fn test_files() {
        let mut path = PathBuf::from(file!());

        path.pop();
        path.pop();
        path.push("tests");

        for test_path in path.read_dir().unwrap() {
            let mut test_path = test_path.unwrap().path();

            if test_path.file_name().unwrap().to_str().unwrap().ends_with(".expected.nix") {
                continue
            }

            let content = fs::read_to_string(&test_path).unwrap();

            // Find pattern
            let mut i = content.find('\n').unwrap();
            let pattern = &content[2..i];

            i += 2;

            // Find replacement / expected text
            let mut replace_by = String::new();

            for line in content.lines().skip(1) {
                if !line.starts_with('#') {
                    break
                }

                replace_by.push_str(&line[2..]);
                i += line.len();
            }

            // Find given text
            let given = &content[i..];

            // Find expected text
            test_path.set_extension("expected.nix");

            if test_path.exists() {
                // Test replacement
                let expected = fs::read_to_string(&test_path).unwrap();

                // Perform replacement
                let cmd = Command::Set {
                    path: pattern.to_string(),
                    value: replace_by
                };

                let mut result = given.to_string();

                process(rnix::parse(given).unwrap(), cmd, &mut result).unwrap();

                // Compare with expected output
                assert_eq!(result.trim(), expected.trim());

            } else {
                // Test query
                let cmd = Command::Get {
                    path: pattern.to_string()
                };

                let mut result = String::new();

                process(rnix::parse(given).unwrap(), cmd, &mut result).unwrap();

                // Compare with expected output
                assert_eq!(result, replace_by);
            }
        }
    }
}
