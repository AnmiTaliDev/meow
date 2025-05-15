use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
struct ColorConfig {
    normal: String,
    number: String,
    highlight: String,
    error: String,
    success: String,
    filename: String,
    reset: String,
}

impl ColorConfig {
    fn new(use_colors: bool) -> Self {
        if use_colors {
            ColorConfig {
                normal: "\x1B[0m".to_string(),
                number: "\x1B[33m".to_string(),  // Yellow
                highlight: "\x1B[36m".to_string(), // Cyan
                error: "\x1B[31m".to_string(),    // Red
                success: "\x1B[32m".to_string(),  // Green
                filename: "\x1B[35m".to_string(), // Magenta
                reset: "\x1B[0m".to_string(),
            }
        } else {
            ColorConfig {
                normal: "".to_string(),
                number: "".to_string(),
                highlight: "".to_string(),
                error: "".to_string(),
                success: "".to_string(),
                filename: "".to_string(),
                reset: "".to_string(),
            }
        }
    }
}

#[derive(Clone)]
struct Config {
    show_line_numbers: bool,
    show_ends: bool,
    show_tabs: bool,
    squeeze_blank: bool,
    number_nonblank: bool,
    show_all_nonprinting: bool,
    show_line_length: bool,
    rainbow_mode: bool,
    use_colors: bool,
    interactive_mode: bool,
    show_meta: bool,
    grep_pattern: Option<String>,
    page_mode: bool,
    animate: bool,
    highlight_pattern: Option<String>,
    files: Vec<String>,
    colors: ColorConfig,
}

impl Config {
    fn new() -> Self {
        let use_colors = atty::is(atty::Stream::Stdout);
        let colors = ColorConfig::new(use_colors);
        
        Config {
            show_line_numbers: false,
            show_ends: false,
            show_tabs: false,
            squeeze_blank: false,
            number_nonblank: false,
            show_all_nonprinting: false,
            show_line_length: false,
            rainbow_mode: false,
            use_colors,
            interactive_mode: false,
            show_meta: false,
            grep_pattern: None,
            page_mode: false,
            animate: false,
            highlight_pattern: None,
            files: Vec::new(),
            colors,
        }
    }
    
    fn parse_args(&mut self, args: &[String]) -> bool {
        let mut i = 1;
        while i < args.len() {
            let arg = &args[i];
            
            if arg.starts_with("--") {
                // Long options
                match arg.as_str() {
                    "--help" => return false,
                    "--number" => self.show_line_numbers = true,
                    "--show-ends" => self.show_ends = true,
                    "--show-tabs" => self.show_tabs = true,
                    "--squeeze-blank" => self.squeeze_blank = true,
                    "--number-nonblank" => self.number_nonblank = true,
                    "--show-nonprinting" => self.show_all_nonprinting = true,
                    "--show-length" => self.show_line_length = true,
                    "--rainbow" => self.rainbow_mode = true,
                    "--no-color" => {
                        self.use_colors = false;
                        self.colors = ColorConfig::new(false);
                    },
                    "--interactive" => self.interactive_mode = true,
                    "--meta" => self.show_meta = true,
                    "--page" => self.page_mode = true,
                    "--animate" => self.animate = true,
                    _ if arg.starts_with("--grep=") => {
                        self.grep_pattern = Some(arg[7..].to_string());
                    },
                    _ if arg.starts_with("--highlight=") => {
                        self.highlight_pattern = Some(arg[12..].to_string());
                    },
                    _ => {
                        eprintln!("{}meow: unknown option: {}{}", self.colors.error, arg, self.colors.reset);
                        return false;
                    }
                }
            } else if arg.starts_with('-') && arg.len() > 1 {
                // Short options
                for c in arg[1..].chars() {
                    match c {
                        'n' => self.show_line_numbers = true,
                        'E' => self.show_ends = true,
                        'T' => self.show_tabs = true,
                        's' => self.squeeze_blank = true,
                        'b' => self.number_nonblank = true,
                        'A' => self.show_all_nonprinting = true,
                        'l' => self.show_line_length = true,
                        'r' => self.rainbow_mode = true,
                        'C' => {
                            self.use_colors = false;
                            self.colors = ColorConfig::new(false);
                        },
                        'i' => self.interactive_mode = true,
                        'm' => self.show_meta = true,
                        'p' => self.page_mode = true,
                        'a' => self.animate = true,
                        'g' => {
                            if i + 1 < args.len() {
                                self.grep_pattern = Some(args[i + 1].clone());
                                i += 1;
                            } else {
                                eprintln!("{}meow: -g requires a pattern{}", self.colors.error, self.colors.reset);
                                return false;
                            }
                        },
                        'H' => {
                            if i + 1 < args.len() {
                                self.highlight_pattern = Some(args[i + 1].clone());
                                i += 1;
                            } else {
                                eprintln!("{}meow: -H requires a pattern{}", self.colors.error, self.colors.reset);
                                return false;
                            }
                        },
                        'h' => return false,
                        _ => {
                            eprintln!("{}meow: unknown option: -{}{}", self.colors.error, c, self.colors.reset);
                            return false;
                        }
                    }
                }
            } else {
                // Files
                self.files.push(arg.clone());
            }
            
            i += 1;
        }
        
        true
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut config = Config::new();
    
    if !config.parse_args(&args) {
        print_help(&config);
        return;
    }
    
    // If no files, read from stdin
    if config.files.is_empty() {
        let stdin = io::stdin();
        process_input(&mut BufReader::new(stdin), &config, "stdin");
    } else {
        // Process each file
        for file_path in &config.files {
            let path = Path::new(file_path);
            match File::open(path) {
                Ok(file) => {
                    if config.files.len() > 1 {
                        println!("\n===> {}{}{}{}{}",
                                config.colors.filename,
                                file_path,
                                config.colors.reset,
                                if config.show_meta { get_file_meta(path) } else { "".to_string() },
                                " <===");
                    }
                    
                    let mut reader = BufReader::new(file);
                    
                    if config.page_mode {
                        let content = read_all_content(&mut reader);
                        page_content(&content);
                    } else {
                        process_input(&mut reader, &config, file_path);
                    }
                },
                Err(err) => {
                    eprintln!("{}meow: {}: {}{}", config.colors.error, file_path, err, config.colors.reset);
                }
            }
        }
    }
    
    // Interactive mode prompt after all files are processed
    if config.interactive_mode {
        interactive_shell(&config);
    }
}

fn read_all_content<R: Read>(reader: &mut BufReader<R>) -> String {
    let mut content = String::new();
    if let Err(e) = reader.read_to_string(&mut content) {
        eprintln!("Error reading content: {}", e);
    }
    content
}

fn page_content(content: &str) {
    let mut pager = Command::new("less")
        .stdin(Stdio::piped())
        .spawn()
        .expect("Failed to start pager");
    
    {
        let stdin = pager.stdin.as_mut().expect("Failed to open stdin");
        stdin.write_all(content.as_bytes()).expect("Failed to write to stdin");
    }
    
    pager.wait().expect("Failed to wait on pager");
}

fn get_file_meta(path: &Path) -> String {
    let metadata = match path.metadata() {
        Ok(meta) => meta,
        Err(_) => return "".to_string(),
    };
    
    let size = metadata.len();
    let size_str = if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    };
    
    let modified = match metadata.modified() {
        Ok(time) => {
            let duration = match time.duration_since(UNIX_EPOCH) {
                Ok(duration) => duration,
                Err(_) => return format!(" [{}]", size_str),
            };
            
            let secs = duration.as_secs();
            let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(now) => now.as_secs(),
                Err(_) => return format!(" [{}]", size_str),
            };
            
            if now - secs < 60 * 60 {
                format!("{} mins ago", (now - secs) / 60)
            } else if now - secs < 60 * 60 * 24 {
                format!("{} hours ago", (now - secs) / (60 * 60))
            } else {
                format!("{} days ago", (now - secs) / (60 * 60 * 24))
            }
        },
        Err(_) => "unknown time".to_string(),
    };
    
    format!(" [{}] [{}]", size_str, modified)
}

fn process_input<R: Read>(reader: &mut BufReader<R>, config: &Config, file_name: &str) {
    // Check if we need to animate the output
    if config.animate {
        let content = read_all_content(reader);
        animate_text(&content);
        return;
    }
    
    let mut line_num = 0;
    let mut prev_blank = false;
    
    let mut lines = reader.lines();
    while let Some(line_result) = lines.next() {
        match line_result {
            Ok(line) => {
                let is_blank = line.trim().is_empty();
                
                // Skip blank lines with squeeze_blank option
                if config.squeeze_blank && is_blank && prev_blank {
                    continue;
                }
                
                // Skip lines that don't match the grep pattern
                if let Some(pattern) = &config.grep_pattern {
                    if !line.contains(pattern) {
                        continue;
                    }
                }
                
                prev_blank = is_blank;
                
                // Handle line numbering
                if config.number_nonblank {
                    if !is_blank {
                        line_num += 1;
                        print!("{}{:6}{} | ", config.colors.number, line_num, config.colors.reset);
                    } else {
                        print!("       | ");
                    }
                } else if config.show_line_numbers {
                    line_num += 1;
                    print!("{}{:6}{} | ", config.colors.number, line_num, config.colors.reset);
                }
                
                // Process and print the line
                let mut output_line = String::new();
                
                if config.show_all_nonprinting {
                    // Show non-printing characters
                    for c in line.chars() {
                        if c.is_control() && c != '\t' {
                            output_line.push('^');
                            output_line.push((c as u8 + 64) as char);
                        } else if c == '\t' && config.show_tabs {
                            output_line.push_str("^I");
                        } else {
                            output_line.push(c);
                        }
                    }
                } else {
                    // Normal printing with tab handling
                    if config.show_tabs {
                        output_line = line.replace('\t', "^I");
                    } else {
                        output_line = line;
                    }
                }
                
                // Highlight pattern if specified
                if let Some(pattern) = &config.highlight_pattern {
                    if output_line.contains(pattern) {
                        let parts: Vec<&str> = output_line.split(pattern).collect();
                        print!("{}", parts[0]);
                        
                        for i in 1..parts.len() {
                            print!("{}{}{}{}", config.colors.highlight, pattern, config.colors.reset, parts[i]);
                        }
                    } else {
                        print!("{}", output_line);
                    }
                } else if config.rainbow_mode {
                    // Rainbow mode - colorize each character
                    let rainbow_colors = [
                        "\x1B[31m", "\x1B[33m", "\x1B[32m", "\x1B[36m", "\x1B[34m", "\x1B[35m",
                    ];
                    
                    for (i, c) in output_line.chars().enumerate() {
                        let color_index = i % rainbow_colors.len();
                        print!("{}{}{}", rainbow_colors[color_index], c, config.colors.reset);
                    }
                } else {
                    print!("{}", output_line);
                }
                
                // Show line length if requested
                if config.show_line_length {
                    print!(" {}[{}L, {}C]{}", 
                           config.colors.normal, 
                           output_line.lines().count(), 
                           output_line.chars().count(),
                           config.colors.reset);
                }
                
                // Show end of line marker
                if config.show_ends {
                    print!("{}${}",
                          if config.use_colors { config.colors.highlight.clone() } else { "".to_string() },
                          config.colors.reset);
                }
                
                println!();
            },
            Err(err) => {
                eprintln!("{}meow: {}: {}{}", config.colors.error, file_name, err, config.colors.reset);
                break;
            }
        }
    }
}

fn animate_text(content: &str) {
    for line in content.lines() {
        for c in line.chars() {
            print!("{}", c);
            io::stdout().flush().unwrap();
            thread::sleep(Duration::from_millis(10));
        }
        println!();
        thread::sleep(Duration::from_millis(50));
    }
}

fn interactive_shell(config: &Config) {
    let mut command_history: Vec<String> = Vec::new();
    let current_config = config.clone();
    
    println!("\n{}=== Meow Interactive Shell ==={}", config.colors.success, config.colors.reset);
    println!("Type 'help' for available commands, 'exit' to quit\n");
    
    loop {
        print!("{}meow>{} ", config.colors.success, config.colors.reset);
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }
        
        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        
        command_history.push(input.to_string());
        
        let parts: Vec<&str> = input.split_whitespace().collect();
        
        if parts.is_empty() {
            continue;
        }
        
        match parts[0] {
            "exit" | "quit" => break,
            "help" => {
                println!("Available commands:");
                println!("  cat <file>    - Display file contents");
                println!("  grep <pattern> <file> - Find pattern in file");
                println!("  highlight <pattern> <file> - Highlight pattern in file");
                println!("  rainbow <file> - Display file with rainbow colors");
                println!("  history       - Show command history");
                println!("  exit/quit     - Exit the shell");
            },
            "cat" => {
                if parts.len() < 2 {
                    println!("{}Usage: cat <file>{}", config.colors.error, config.colors.reset);
                    continue;
                }
                
                if let Ok(file) = File::open(parts[1]) {
                    let mut reader = BufReader::new(file);
                    process_input(&mut reader, &current_config, parts[1]);
                } else {
                    println!("{}Error: Could not open file '{}'{}", config.colors.error, parts[1], config.colors.reset);
                }
            },
            "grep" => {
                if parts.len() < 3 {
                    println!("{}Usage: grep <pattern> <file>{}", config.colors.error, config.colors.reset);
                    continue;
                }
                
                if let Ok(file) = File::open(parts[2]) {
                    let mut local_config = current_config.clone();
                    local_config.grep_pattern = Some(parts[1].to_string());
                    let mut reader = BufReader::new(file);
                    process_input(&mut reader, &local_config, parts[2]);
                } else {
                    println!("{}Error: Could not open file '{}'{}", config.colors.error, parts[2], config.colors.reset);
                }
            },
            "highlight" => {
                if parts.len() < 3 {
                    println!("{}Usage: highlight <pattern> <file>{}", config.colors.error, config.colors.reset);
                    continue;
                }
                
                if let Ok(file) = File::open(parts[2]) {
                    let mut local_config = current_config.clone();
                    local_config.highlight_pattern = Some(parts[1].to_string());
                    let mut reader = BufReader::new(file);
                    process_input(&mut reader, &local_config, parts[2]);
                } else {
                    println!("{}Error: Could not open file '{}'{}", config.colors.error, parts[2], config.colors.reset);
                }
            },
            "rainbow" => {
                if parts.len() < 2 {
                    println!("{}Usage: rainbow <file>{}", config.colors.error, config.colors.reset);
                    continue;
                }
                
                if let Ok(file) = File::open(parts[1]) {
                    let mut local_config = current_config.clone();
                    local_config.rainbow_mode = true;
                    let mut reader = BufReader::new(file);
                    process_input(&mut reader, &local_config, parts[1]);
                } else {
                    println!("{}Error: Could not open file '{}'{}", config.colors.error, parts[1], config.colors.reset);
                }
            },
            "history" => {
                println!("Command history:");
                for (i, cmd) in command_history.iter().enumerate() {
                    println!("  {}. {}", i + 1, cmd);
                }
            },
            _ => {
                println!("{}Unknown command: '{}'{}", config.colors.error, parts[0], config.colors.reset);
                println!("Type 'help' to see available commands");
            }
        }
    }
}

fn print_help(config: &Config) {
    println!("{}Usage:{} meow [OPTIONS]... [FILE]...", config.colors.success, config.colors.reset);
    println!("Concatenate FILE(s) to standard output with enhancements.");
    println!();
    println!("If FILE is not specified or is -, read standard input.");
    println!();
    println!("  -n, --number             number all output lines");
    println!("  -b, --number-nonblank    number nonempty output lines");
    println!("  -E, --show-ends          display $ at end of each line");
    println!("  -T, --show-tabs          display TAB characters as ^I");
    println!("  -s, --squeeze-blank      suppress repeated empty output lines");
    println!("  -A, --show-nonprinting   show all non-printing characters");
    println!("  -l, --show-length        show line and character count");
    println!("  -r, --rainbow            enable rainbow text mode");
    println!("  -C, --no-color           disable colors");
    println!("  -i, --interactive        enter interactive mode after processing");
    println!("  -m, --meta               show file metadata");
    println!("  -p, --page               use pager (like less) for output");
    println!("  -a, --animate            animate text display");
    println!("  -g <pattern>, --grep=<pattern>    only show lines matching pattern");
    println!("  -H <pattern>, --highlight=<pattern>  highlight pattern in output");
    println!("  -h, --help               display this help and exit");
    println!();
    println!("Examples:");
    println!("  meow -n file.txt            Display file with line numbers");
    println!("  meow -ET file.txt           Show tabs and line endings");
    println!("  meow -g 'pattern' file.txt  Only show lines matching 'pattern'");
    println!("  meow -r file.txt            Display rainbow text");
    println!();
    println!("Report bugs to: github.com/anmitalidev/meow");
}