use color_eyre::Result;
use crossterm::{
    execute,
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
    cursor::{Show}, 
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Rect, Layout, Constraint, Direction, Alignment},
    style::{Style, Color, Modifier},
    widgets::{Block, Borders, Paragraph, List, ListItem, ListState, Clear}, 
    Frame,
    Terminal,
    text::{Line, Span}
};
use std::io::{self, stdout};
use std::env;
use std::fs;
use std::collections::HashMap;
use std::process::Command;
use std::path::Path; 

const DE_DM_MAP: &[(&str, &str)] = &[
    ("KDE-Desktop", "sddm"),
    ("GNOME-Desktop", "gdm"),
    ("XFCE4-Desktop", "lightdm"),
    ("Cinnamon-Desktop", "lightdm"),
    ("MATE-Desktop", "lightdm"),
    ("Budgie-Desktop", "lightdm"),
    ("LXQT-Desktop", "sddm"),
    ("LXDE-Desktop", "lightdm"),
    ("i3-Window-Manager", "lightdm"),
];

lazy_static::lazy_static! {
    static ref SPECIAL_INSTALL_MAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("COSMIC-Desktop", "cosmic");
        m.insert("i3-Window-Manager", "i3-gaps");
        m
    };
}

const PKG_MANAGER_LIST: &[&str] = &["pacman", "yay", "paru"];

fn map_raw_de_to_profile(raw_de: &str) -> String {
    match raw_de.to_uppercase().as_str() {
        "COSMIC" => return "COSMIC-Desktop".to_string(),
        "I3" => return "i3-Window-Manager".to_string(),
        _ => {}
    }

    DE_DM_MAP.iter()
        .find(|(profile, _)| profile.starts_with(&raw_de.to_uppercase()))
        .map(|(profile, _)| profile.to_string())
        .unwrap_or_else(|| "Unknown-Desktop".to_string())
}

fn get_available_des() -> Result<Vec<String>> {
    let output = Command::new("eos-packagelist")
        .arg("--list")
        .output()?;
    
    if !output.status.success() {
        return Ok(DE_DM_MAP.iter().map(|(d, _)| d.to_string()).collect());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    let available_des: Vec<String> = stdout.lines()
        .filter(|line| {
            let line = line.trim();
            line.ends_with("-Desktop") || line.ends_with("-Window-Manager") || line.contains("i3")
        })
        .map(|line| line.trim().to_string())
        .collect();

    if available_des.is_empty() {
        Ok(DE_DM_MAP.iter().map(|(d, _)| d.to_string()).collect())
    } else {
        Ok(available_des)
    }

}

#[derive(Debug, PartialEq)]
pub enum AppStep {
    SelectDE,
    InputPath
}

pub struct App {
    pub current_de_raw: String,
    pub current_de_profile: String,
    pub available_des: Vec<String>,
    pub selected_de_index: usize,
    pub selected_pkg_manager_index: usize,
    pub should_quit: bool,
    pub current_step: AppStep,
    pub input_buffer: String,
    pub input_cursor_position: usize,
    pub input_error: Option<String>
}

impl App {
    pub fn new() -> Result<Self> {
        let current_de_raw = env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_else(|_| "Unknown".to_string())
            .split(':').last().unwrap_or("Unknown").to_string();

        let current_de_profile = map_raw_de_to_profile(&current_de_raw);
        let available_des = get_available_des()?;
        let initial_path = format!("./{}", App::generate_initial_filename(&current_de_profile, &available_des[0]));
        
        Ok(App {
            current_de_raw,
            current_de_profile,
            available_des,
            selected_de_index: 0,
            selected_pkg_manager_index: 0, 
            should_quit: false,
            current_step: AppStep::SelectDE,
            input_buffer: initial_path.clone(),
            input_cursor_position: initial_path.len(),
            input_error: None
        })
    }

    fn generate_initial_filename(from_profile: &str, to_profile: &str) -> String {
        let from = from_profile.replace("-Desktop", "").replace("-Window-Manager", "");
        let to = to_profile.replace("-Desktop", "").replace("-Window-Manager", "");
        
        if from == "Unknown-Desktop" {
            format!("de_switcher_from_Unknown_to_{}.sh", to)
        } else {
            format!("de_switcher_{}_to_{}.sh", from.replace("-Desktop", ""), to)
        }
    }
    
    pub fn update_filename_on_de_change(&mut self) {
        let new_filename = App::generate_initial_filename(
            &self.current_de_profile,
            &self.available_des[self.selected_de_index]
        );
        
        if self.current_step == AppStep::SelectDE {
            self.input_buffer = format!("./{}", new_filename);
            self.input_cursor_position = self.input_buffer.len();
        }
    }

    pub fn next_de(&mut self) {
        self.selected_de_index = (self.selected_de_index + 1) % self.available_des.len();
        self.update_filename_on_de_change();
    }

    pub fn previous_de(&mut self) {
        if self.selected_de_index > 0 {
            self.selected_de_index -= 1;
        } else {
            self.selected_de_index = self.available_des.len() - 1;
        }
        self.update_filename_on_de_change();
    }
    
    pub fn cycle_pkg_manager(&mut self) {
        self.selected_pkg_manager_index = (self.selected_pkg_manager_index + 1) % PKG_MANAGER_LIST.len();
    }

    pub fn generate_filename(&self) -> String {
        self.input_buffer.clone() 
    }

    pub fn generate_script(&self) -> String {

        let current_de_profile_for_removal = &self.current_de_profile;
        let target_de_profile = &self.available_des[self.selected_de_index];
        let pkg_manager = PKG_MANAGER_LIST[self.selected_pkg_manager_index];
        let script_file_placeholder = "de_switch_script.sh";
        let sudo_cmd = if pkg_manager == "pacman" { "sudo" } else { "" };
        let sudo_remove_cmd = if pkg_manager == "pacman" { "sudo" } else { "" };

        let target_dm = DE_DM_MAP.iter()
            .find(|(profile, _dm)| profile == target_de_profile)
            .map(|(_profile, dm)| *dm)
            .unwrap_or("lightdm");

        let sudo_space = if sudo_cmd.is_empty() { "" } else { " " };
        let special_install_cmd = if let Some(pkg_group) = SPECIAL_INSTALL_MAP.get(target_de_profile.as_str()) {
            format!("echo \"Installing special package group: {}\"\n{}{}{} -S {}\n", pkg_group, sudo_cmd, sudo_space, pkg_manager, pkg_group)
        } else {
            format!("echo \"Installing packages for {} using eos-packagelist...\"\n{}{}{} -S $(eos-packagelist --install \"{}\")\n", target_de_profile, sudo_cmd, sudo_space, pkg_manager, target_de_profile)
        };
        
        let sudo_remove_cmd_with_space = if sudo_remove_cmd.is_empty() { "" } else { " " };

        format!(
            r#"#!/bin/bash
# ----------------------------------------------------
# Generated by Rust DE Switcher TUI
# Target DE: {}
# Package Manager: {}
#
# REVIEW THIS SCRIPT BEFORE RUNNING:
# bash {}
# ----------------------------------------------------
echo "Preparing to switch from {} to {} using {}..."

# 1. REMOVE CURRENT DE PACKAGES
# This assumes the current DE profile is one of the recognized eos-packagelist profiles.
# CAUTION: This operation removes package dependencies recursively.

CURRENT_DE_PROFILE="{}"

if [ -n "$CURRENT_DE_PROFILE" ] && [ "$CURRENT_DE_PROFILE" != "Unknown-Desktop" ] && [ "$CURRENT_DE_PROFILE" != "{}" ]; then
    echo "Creating package list for removal: $CURRENT_DE_PROFILE..."

    # eos-packagelist runs as user
    eos-packagelist "$CURRENT_DE_PROFILE" > /tmp/old_de_packages.txt
    
    echo "Removing old DE packages (may prompt for password)..."
    # -Rcs: Remove, cascade, remove dependencies only required by package(s) being removed
    {}{}{} -Rcs - < /tmp/old_de_packages.txt
    rm /tmp/old_de_packages.txt

else
    echo "Skipping old DE removal (Current DE profile: $CURRENT_DE_PROFILE is Unknown or matches target)."
fi

# 2. INSTALL NEW DE PACKAGES
{}

# 3. ENABLE THE APPROPRIATE DISPLAY MANAGER
echo "Enabling Display Manager: {}"

# Disable any currently enabled display-manager service
sudo systemctl disable --force $(systemctl list-units --type=service --state=enabled --no-pager | grep "display-manager" | awk '{{print $1}}') 2>/dev/null

# Enable the new display manager
sudo systemctl enable {}

# 4. Final message and reboot
echo ""
echo "!!! Installation and configuration complete. !!!"
echo "!!! You MUST reboot now to finish the switch. !!!"

# Prompt for reboot
read -r -p "Do you want to reboot now? [y/N]: " response
case "$response" in
    [yY][eE][sS]|[yY]) 
        sudo reboot
        ;;
    *)
        echo "Please reboot manually to complete the switch."
        ;;
esac
"#,
            target_de_profile,
            pkg_manager,
            script_file_placeholder, 
            current_de_profile_for_removal, 
            target_de_profile, 
            pkg_manager,
            current_de_profile_for_removal, 
            target_de_profile, 
            sudo_remove_cmd, 
            sudo_remove_cmd_with_space, 
            pkg_manager,
            special_install_cmd,
            target_dm,
            target_dm
        )

    }
    
    pub fn validate_and_finalize_path(&mut self) -> bool {
        let p = Path::new(&self.input_buffer);
        
        if p.is_dir() {
            self.input_error = Some("Path cannot be a directory. Please provide a filename.".to_string());
            return false;
        }

        let parent = p.parent().unwrap_or(Path::new(""));

        if parent.to_string_lossy() != "" && !parent.exists() {
             self.input_error = Some("Directory does not exist.".to_string());
             return false;
        }

        if p.file_name().is_none() || p.file_name().unwrap().to_string_lossy().is_empty() {
             self.input_error = Some("Filename cannot be empty.".to_string());
             return false;
        }

        self.input_error = None;
        self.should_quit = true;
        true
    }
}

fn render_path_input(f: &mut Frame, _area: Rect, app: &mut App) {
    let area = f.area();
    let style = Style::default().fg(Color::White).bg(Color::Black);
    let error_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 3), 
            Constraint::Length(5),   
            Constraint::Min(0),      
        ])
        .split(area);
        
    let centered_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 4), 
            Constraint::Ratio(2, 4), 
            Constraint::Ratio(1, 4), 
        ])
        .split(chunks[1]);

    let input_area = centered_chunks[1];
    
    f.render_widget(Clear, input_area); 
    
    let border_color = if app.input_error.is_some() { Color::Red } else { Color::Cyan };

    let input_block = Block::default()
        .title(" Output Script Path (ESC to cancel) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    
    let input_title = "Enter Path and Filename (e.g., /home/user/myscript.sh):";
    let error_msg = app.input_error.as_ref().map_or("", |e| e.as_str());

    let text = vec![
        Line::from(input_title),
        Line::from(""),
        Line::from(app.input_buffer.as_str()),
        Line::from(""),
        Line::from(Span::styled(error_msg, error_style)),
    ];

    let paragraph = Paragraph::new(text)
        .block(input_block)
        .alignment(Alignment::Left)
        .style(style);

    f.render_widget(paragraph, input_area);
}


fn main() -> Result<()> {
    let mut app = match App::new() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error initializing app (could not run eos-packagelist): {}", e);
            return Err(e);
        }
    };
    
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let result = run_app(&mut terminal, &mut app);

    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    execute!(terminal.backend_mut(), Show)?;
    disable_raw_mode()?;

    if let Err(e) = result {
        return Err(e);
    }
    
    if app.should_quit {
        let full_path = app.generate_filename(); 
        let file_name_only = Path::new(&full_path).file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_else(|| "de_switcher.sh".to_string());
        
        let script_content = app.generate_script(); 
        let final_script_content = script_content.replace("de_switch_script.sh", &file_name_only); 

        match fs::write(&full_path, final_script_content) {
            Ok(_) => println!("\nScript successfully written to **{}**\n\n**NEXT STEP: REVIEW AND RUN:**\n\t`chmod +x {}`\n\t`{}`\n", full_path, full_path, full_path),
            Err(e) => eprintln!("\nError writing script file: {}", e),
        }
    }

    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        if app.should_quit {
            return Ok(());
        }

        terminal.draw(|f| {
            let area = f.area();
            render_ui(f, area, app);

            if app.current_step == AppStep::InputPath {
                let input_area = f.area();
                let input_area_x = input_area.width / 4;
                
                let cursor_x = input_area_x + 1 + (app.input_cursor_position as u16);
                let cursor_y = input_area.height / 3 + 3;

                f.set_cursor_position((cursor_x, cursor_y));
            }
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match app.current_step {
                    AppStep::SelectDE => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => app.next_de(),
                        KeyCode::Char('k') | KeyCode::Up => app.previous_de(),
                        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => app.cycle_pkg_manager(),
                        KeyCode::Tab => app.cycle_pkg_manager(),
                        KeyCode::Enter => {
                            app.current_step = AppStep::InputPath;
                            app.input_error = None; 
                        }
                        _ => {}
                    },
                    AppStep::InputPath => match key.code {
                        KeyCode::Char(c) => {
                            app.input_buffer.insert(app.input_cursor_position, c);
                            app.input_cursor_position += 1;
                        }
                        KeyCode::Backspace => {
                            if app.input_cursor_position > 0 {
                                app.input_cursor_position -= 1;
                                app.input_buffer.remove(app.input_cursor_position);
                            }
                        }
                        KeyCode::Delete => {
                            if app.input_cursor_position < app.input_buffer.len() {
                                app.input_buffer.remove(app.input_cursor_position);
                            }
                        }
                        KeyCode::Left => {
                            if app.input_cursor_position > 0 {
                                app.input_cursor_position -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if app.input_cursor_position < app.input_buffer.len() {
                                app.input_cursor_position += 1;
                            }
                        }
                        KeyCode::Enter => {
                            if app.validate_and_finalize_path() {
                                return Ok(()); 
                            }
                        }
                        KeyCode::Esc => {
                            app.current_step = AppStep::SelectDE;
                            app.input_error = None;
                            app.update_filename_on_de_change(); 
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}


fn render_ui(frame: &mut Frame, _area: Rect, app: &mut App) {
    let area = frame.area(); 
    
    if app.current_step == AppStep::InputPath {
        render_path_input(frame, area, app);
        return;
    }
    
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),     
            Constraint::Min(0),        
            Constraint::Length(1),     
        ])
        .split(area);
        
    let top_bar_area = vertical_chunks[0];
    let main_area = vertical_chunks[1];
    let footer_area = vertical_chunks[2];

    let header_title = format!(" de-switcher | Quickly switch desktop environments using eos-packagelist. ");
    let header_block = Block::default()
        .title(header_title)
        .title_alignment(Alignment::Left)
        .borders(Borders::TOP | Borders::RIGHT | Borders::LEFT )
        .border_style(Style::default().fg(Color::Yellow));

    frame.render_widget(header_block, top_bar_area);

    let footer_title = " bladeacer | Copyright (c) 2025 ";
    let footer_block = Block::default()
        .title(footer_title)
        .title_alignment(Alignment::Right)
        .borders(Borders::BOTTOM | Borders::RIGHT | Borders::LEFT )
        .border_style(Style::default().fg(Color::Yellow));

    frame.render_widget(footer_block, footer_area);

    let main_content_block_with_borders = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .padding(ratatui::widgets::Padding::new(1, 1, 0, 0));
        
    frame.render_widget(main_content_block_with_borders.clone(), main_area);
    
    let inner_main_area = main_content_block_with_borders.inner(main_area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ])
        .split(inner_main_area); 

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 2), 
            Constraint::Length(1),   
            Constraint::Ratio(1, 2), 
        ])
        .split(chunks[0]);
    
    let list_area = top_chunks[0];
    let info_and_pkg_area = top_chunks[2];

    let info_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 2), 
            Constraint::Ratio(1, 2), 
        ])
        .split(info_and_pkg_area);


    let info_block = Block::default()
        .title(" Info ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));
    
    let info_text = format!(
        "Current DE: **{}**\nProfile: **{}**\n\n\
         Use **j/k** or Up/Down to select a target DE.\n\
         Press **Ctrl+P** or **Tab** to change the Package Manager.\n\
         Press **<ENTER>** to set output path.", 
        app.current_de_raw,
        app.current_de_profile
    );

    let info_paragraph = Paragraph::new(info_text).block(info_block);
    frame.render_widget(info_paragraph, info_chunks[0]);
    
    let current_pkg_manager = PKG_MANAGER_LIST[app.selected_pkg_manager_index];
    let pkg_manager_block = Block::default()
        .title(" Package Manager (Ctrl+P/Tab to cycle) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let pkg_manager_text = format!(
        "Selected: **{}**\n\n\
         Note: {} is used for installation commands, e.g., `{} -S ...`",
        current_pkg_manager,
        current_pkg_manager,
        current_pkg_manager
    );
    
    let pkg_manager_paragraph = Paragraph::new(pkg_manager_text).block(pkg_manager_block);
    frame.render_widget(pkg_manager_paragraph, info_chunks[1]);


    let items: Vec<ListItem> = app.available_des.iter()
        .map(|de| {
            let style = if de == &app.current_de_profile { 
                Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM) 
            } else { 
                Style::default().fg(Color::White) 
            };
            ListItem::new(de.as_str()).style(style)
        })
        .collect();

    let list_block = Block::default()
        .title(" Available DE Profiles (Target DE) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let list = List::new(items)
        .block(list_block)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))
        .highlight_symbol(">> ");

    let mut list_state = ListState::default();
    list_state.select(Some(app.selected_de_index));

    frame.render_stateful_widget(list, list_area, &mut list_state);


    let selected_de_name = &app.available_des[app.selected_de_index];
    let script_content = app.generate_script();
    
    let script_block = Block::default()
        .title(format!(" Script Preview for: {} ", selected_de_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let preview_text: String = script_content.lines().take(30).collect::<Vec<&str>>().join("\n");
    let script_paragraph = Paragraph::new(preview_text).block(script_block);

    frame.render_widget(script_paragraph, chunks[1]);
}
