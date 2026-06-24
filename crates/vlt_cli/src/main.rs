use clap::{Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use vlt_compiler::ai::{
    ai_prompt, ai_summary, explain_symbol, index_project, plan_task, AiPromptFormat,
};
use vlt_compiler::project::{compile_file, run_file, ProjectError};
use vlt_compiler::scaffold::create_api_project;
use vlt_compiler::{check_program, format_program, parse_source, SourceFile};

#[derive(Debug, Parser)]
#[command(name = "vlt")]
#[command(about = "Volt language compiler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    New {
        #[command(subcommand)]
        command: NewCommand,
    },
    Ai {
        #[command(subcommand)]
        command: AiCommand,
    },
    Explain {
        symbol: String,
    },
    Plan {
        task: String,
    },
    Parse {
        file: PathBuf,
    },
    Check {
        file: PathBuf,
    },
    Build {
        file: PathBuf,
    },
    Run {
        file: PathBuf,
    },
    Fmt {
        file: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum NewCommand {
    Api { name: String },
}

#[derive(Debug, Subcommand)]
enum AiCommand {
    Index,
    Summary,
    Prompt {
        task: String,
        #[arg(long, value_enum, default_value = "generic")]
        format: PromptFormatArg,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PromptFormatArg {
    Generic,
    Codex,
    Claude,
}

impl From<PromptFormatArg> for AiPromptFormat {
    fn from(value: PromptFormatArg) -> Self {
        match value {
            PromptFormatArg::Generic => AiPromptFormat::Generic,
            PromptFormatArg::Codex => AiPromptFormat::Codex,
            PromptFormatArg::Claude => AiPromptFormat::Claude,
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

fn run() -> Result<(), ()> {
    let cli = Cli::parse();
    match cli.command {
        Command::New { command } => match command {
            NewCommand::Api { name } => {
                let cwd = std::env::current_dir().map_err(|err| {
                    eprintln!("failed to read current directory: {err}");
                })?;
                match create_api_project(&cwd, &name) {
                    Ok(path) => {
                        println!("created Volt API project: {}", path.display());
                        Ok(())
                    }
                    Err(err) => {
                        eprintln!("failed to create project: {err}");
                        Err(())
                    }
                }
            }
        },
        Command::Ai { command } => {
            let cwd = current_dir()?;
            match command {
                AiCommand::Index => match index_project(&cwd) {
                    Ok(index) => {
                        println!(
                            "indexed project: {} types, {} functions, {} routes",
                            index.types.len(),
                            index.functions.len(),
                            index.routes.len()
                        );
                        Ok(())
                    }
                    Err(err) => {
                        eprintln!("failed to index project: {err}");
                        Err(())
                    }
                },
                AiCommand::Summary => match ai_summary(&cwd) {
                    Ok(summary) => {
                        println!("{summary}");
                        Ok(())
                    }
                    Err(err) => {
                        eprintln!("failed to summarize project: {err}");
                        Err(())
                    }
                },
                AiCommand::Prompt {
                    task,
                    format,
                    output,
                } => match ai_prompt(&cwd, &task, format.into()) {
                    Ok(prompt) => {
                        if let Some(output) = output {
                            match std::fs::write(&output, prompt) {
                                Ok(()) => {
                                    println!("AI prompt written to {}", output.display());
                                    Ok(())
                                }
                                Err(err) => {
                                    eprintln!("failed to write prompt: {err}");
                                    Err(())
                                }
                            }
                        } else {
                            println!("{prompt}");
                            Ok(())
                        }
                    }
                    Err(err) => {
                        eprintln!("failed to generate AI prompt: {err}");
                        Err(())
                    }
                },
            }
        }
        Command::Explain { symbol } => {
            let cwd = current_dir()?;
            match explain_symbol(&cwd, &symbol) {
                Ok(Some(explanation)) => {
                    println!("{explanation}");
                    Ok(())
                }
                Ok(None) => {
                    eprintln!("symbol not found: {symbol}");
                    Err(())
                }
                Err(err) => {
                    eprintln!("failed to explain symbol: {err}");
                    Err(())
                }
            }
        }
        Command::Plan { task } => {
            let cwd = current_dir()?;
            match plan_task(&cwd, &task) {
                Ok(plan) => {
                    println!("{plan}");
                    Ok(())
                }
                Err(err) => {
                    eprintln!("failed to plan task: {err}");
                    Err(())
                }
            }
        }
        Command::Parse { file } => {
            let source = read_source(&file)?;
            match parse_source(&source) {
                Ok(program) => {
                    println!("{program:#?}");
                    Ok(())
                }
                Err(diagnostics) => {
                    eprintln!("{}", diagnostics.render(&source));
                    Err(())
                }
            }
        }
        Command::Check { file } => {
            let source = read_source(&file)?;
            let program = match parse_source(&source) {
                Ok(program) => program,
                Err(diagnostics) => {
                    eprintln!("{}", diagnostics.render(&source));
                    return Err(());
                }
            };
            match check_program(&program, &source) {
                Ok(()) => {
                    println!("check succeeded");
                    Ok(())
                }
                Err(diagnostics) => {
                    eprintln!("{}", diagnostics.render(&source));
                    Err(())
                }
            }
        }
        Command::Build { file } => match compile_file(&file, &output_root()) {
            Ok(output) => {
                println!("generated Rust: {}", output.rust_file.display());
                println!("built binary: {}", output.binary.display());
                Ok(())
            }
            Err(error) => report_project_error(&file, error),
        },
        Command::Run { file } => match run_file(&file, &output_root()) {
            Ok(stdout) => {
                print!("{stdout}");
                Ok(())
            }
            Err(error) => report_project_error(&file, error),
        },
        Command::Fmt { file } => {
            let source = read_source(&file)?;
            match parse_source(&source) {
                Ok(program) => {
                    print!("{}", format_program(&program));
                    Ok(())
                }
                Err(diagnostics) => {
                    eprintln!("{}", diagnostics.render(&source));
                    Err(())
                }
            }
        }
    }
}

fn read_source(file: &Path) -> Result<SourceFile, ()> {
    SourceFile::from_path(file).map_err(|err| {
        eprintln!("failed to read {}: {err}", file.display());
    })
}

fn output_root() -> PathBuf {
    PathBuf::from("target").join("volt")
}

fn current_dir() -> Result<PathBuf, ()> {
    std::env::current_dir().map_err(|err| {
        eprintln!("failed to read current directory: {err}");
    })
}

fn report_project_error(file: &Path, error: ProjectError) -> Result<(), ()> {
    match error {
        ProjectError::Diagnostics(diagnostics) => match SourceFile::from_path(file) {
            Ok(source) => eprintln!("{}", diagnostics.render(&source)),
            Err(_) => eprintln!("{diagnostics}"),
        },
        other => eprintln!("{other}"),
    }
    Err(())
}
