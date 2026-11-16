use crate::EnchantmentInfo;
use eframe::egui;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const FILE_HEADER: &str = "MCHELPFILE0.0.0";

pub fn looper(dir: PathBuf, enchs: Vec<EnchantmentInfo>) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 650.0])
            .with_min_inner_size([650.0, 420.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Minecraft Helper",
        options,
        Box::new(move |_creation_context| Ok(Box::new(WorldApp::new(dir, enchs)))),
    )
}

struct WorldApp {
    dir: PathBuf,
    enchs: Vec<EnchantmentInfo>,
    worlds: Vec<String>,
    world_name: String,
    current_world: Option<String>,
    pending_delete: Option<String>,
    enchantments: Vec<Option<(u8, u8)>>,
    is_new_world: bool,
    dirty: bool,
    error: Option<String>,
}

impl WorldApp {
    fn new(dir: PathBuf, enchs: Vec<EnchantmentInfo>) -> Self {
        let mut app = Self {
            dir,
            enchantments: vec![None; enchs.len()],
            enchs,
            worlds: Vec::new(),
            world_name: String::new(),
            current_world: None,
            pending_delete: None,
            is_new_world: false,
            dirty: false,
            error: None,
        };
        app.refresh_worlds();
        app
    }

    fn refresh_worlds(&mut self) {
        match fs::read_dir(&self.dir) {
            Ok(entries) => {
                let result = entries
                    .map(|entry| entry.map_err(|error| error.to_string()))
                    .collect::<Result<Vec<_>, _>>();
                match result {
                    Ok(entries) => {
                        self.worlds = entries
                            .into_iter()
                            .filter(|entry| entry.path().is_file())
                            .filter_map(|entry| entry.file_name().into_string().ok())
                            .collect();
                        self.worlds.sort_by_key(|name| name.to_lowercase());
                        self.error = None;
                    }
                    Err(error) => {
                        self.error = Some(format!("Could not list worlds: {error}"));
                    }
                }
            }
            Err(error) => self.error = Some(format!("Could not list worlds: {error}")),
        }
    }

    fn open_world(&mut self, name: &str) {
        match load_world(&self.dir.join(name), self.enchs.len()) {
            Ok(enchantments) => {
                self.current_world = Some(name.to_owned());
                self.enchantments = enchantments;
                self.is_new_world = false;
                self.dirty = false;
                self.error = None;
            }
            Err(error) => self.error = Some(format!("Could not open \"{name}\": {error}")),
        }
    }

    fn create_world(&mut self) {
        let name = self.world_name.trim();
        if !is_valid_world_name(name) {
            self.error =
                Some("Enter a world name without path separators or reserved characters.".into());
            return;
        }
        if self.dir.join(name).exists() {
            self.error = Some(format!("A world named \"{name}\" already exists."));
            return;
        }

        self.current_world = Some(name.to_owned());
        self.enchantments = vec![None; self.enchs.len()];
        self.is_new_world = true;
        self.dirty = true;
        self.world_name.clear();
        self.error = None;
    }

    fn save_world(&mut self) {
        let Some(name) = self.current_world.as_deref() else {
            return;
        };
        let path = self.dir.join(name);
        match save_world(&path, &self.enchantments, self.is_new_world) {
            Ok(()) => {
                self.is_new_world = false;
                self.dirty = false;
                self.error = None;
                self.refresh_worlds();
            }
            Err(error) => self.error = Some(format!("Could not save \"{name}\": {error}")),
        }
    }

    fn delete_world(&mut self, name: &str) {
        match fs::remove_file(self.dir.join(name)) {
            Ok(()) => {
                if self.current_world.as_deref() == Some(name) {
                    self.current_world = None;
                    self.enchantments = vec![None; self.enchs.len()];
                    self.is_new_world = false;
                    self.dirty = false;
                }
                self.refresh_worlds();
                self.error = None;
            }
            Err(error) => {
                self.error = Some(format!("Could not delete \"{name}\": {error}"));
            }
        }
    }
}

impl eframe::App for WorldApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(context, |ui| {
            ui.heading("Minecraft Helper");
        });

        egui::SidePanel::left("worlds")
            .resizable(true)
            .default_width(235.0)
            .show(context, |ui| {
                ui.heading("Worlds");
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!self.dirty, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.world_name)
                                .hint_text("New world name"),
                        );
                        if ui.button("Add").clicked() {
                            self.create_world();
                        }
                    });
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for name in self.worlds.clone() {
                        let selected = self.current_world.as_deref() == Some(name.as_str());
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    !self.dirty,
                                    egui::SelectableLabel::new(selected, &name),
                                )
                                .clicked()
                            {
                                self.open_world(&name);
                            }
                            if ui
                                .add_enabled(!self.dirty, egui::Button::new("Delete"))
                                .clicked()
                            {
                                self.pending_delete = Some(name.clone());
                            }
                        });
                    }
                });
                if self.dirty {
                    ui.label("Save your changes before switching or deleting worlds.");
                }
            });

        if let Some(name) = self.pending_delete.clone() {
            egui::Window::new("Delete world?")
                .collapsible(false)
                .resizable(false)
                .show(context, |ui| {
                    ui.label(format!(
                        "Permanently delete \"{name}\"? This cannot be undone."
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.delete_world(&name);
                            self.pending_delete = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending_delete = None;
                        }
                    });
                });
        }

        egui::CentralPanel::default().show(context, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
                ui.add_space(8.0);
            }

            let Some(name) = self.current_world.clone() else {
                ui.heading("Select or add a world");
                return;
            };

            ui.horizontal(|ui| {
                ui.heading(&name);
                if self.dirty {
                    ui.label("(unsaved)");
                }
                if ui
                    .add_enabled(self.dirty, egui::Button::new("Save"))
                    .clicked()
                {
                    self.save_world();
                }
            });
            ui.separator();
            ui.label("Enable enchantments and set their cost and level.");
            ui.add_space(6.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("enchantment_grid")
                    .num_columns(4)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Enchantment");
                        ui.strong("Cost");
                        ui.strong("Level");
                        ui.strong("Maximum");
                        ui.end_row();

                        for (info, value) in self.enchs.iter().zip(&mut self.enchantments) {
                            let mut enabled = value.is_some();
                            if ui
                                .checkbox(&mut enabled, &info.name)
                                .on_hover_text(&info.id)
                                .changed()
                            {
                                *value = enabled.then_some((0, 1));
                                self.dirty = true;
                            }

                            if let Some((cost, level)) = value {
                                if ui
                                    .add(egui::DragValue::new(cost).range(0..=u8::MAX))
                                    .changed()
                                {
                                    self.dirty = true;
                                }
                                let mut level_changed = false;
                                egui::ComboBox::from_id_salt(("level", &info.id))
                                    .selected_text(roman_numeral(*level))
                                    .show_ui(ui, |ui| {
                                        for candidate in 1..=info.max_level {
                                            level_changed |= ui
                                                .selectable_value(
                                                    level,
                                                    candidate,
                                                    roman_numeral(candidate),
                                                )
                                                .changed();
                                        }
                                    });
                                if level_changed {
                                    self.dirty = true;
                                }
                                ui.label(roman_numeral(info.max_level));
                            } else {
                                ui.label("-");
                                ui.label("-");
                                ui.label(roman_numeral(info.max_level));
                            }
                            ui.end_row();
                        }
                    });
            });
        });
    }
}

fn is_valid_world_name(name: &str) -> bool {
    !name.is_empty()
        && name
            == Path::new(name)
                .file_name()
                .and_then(|part| part.to_str())
                .unwrap_or("")
        && !name.chars().any(|character| {
            matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'
            )
        })
}

fn roman_numeral(mut value: u8) -> String {
    let mut numeral = String::new();
    for (number, symbol) in [
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ] {
        while value >= number {
            numeral.push_str(symbol);
            value -= number;
        }
    }
    if numeral.is_empty() {
        numeral.push('-');
    }
    numeral
}

fn load_world(path: &Path, enchantment_count: usize) -> Result<Vec<Option<(u8, u8)>>, String> {
    let contents = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut lines = contents.lines();
    let header = lines.next().ok_or("file is empty")?;
    if !header.starts_with("MCHELPFILE") {
        return Err("unrecognized world file format".into());
    }

    let costs = parse_values(lines.next().unwrap_or_default(), enchantment_count)?;
    let levels = parse_values(lines.next().unwrap_or_default(), enchantment_count)?;
    Ok(costs
        .into_iter()
        .zip(levels)
        .map(|(cost, level)| match (cost, level) {
            (Some(cost), Some(level)) => Some((cost, level)),
            _ => None,
        })
        .collect())
}

fn parse_values(line: &str, count: usize) -> Result<Vec<Option<u8>>, String> {
    let mut values = line
        .split(',')
        .map(|value| {
            if value.is_empty() {
                Ok(None)
            } else {
                value
                    .parse::<u8>()
                    .map(Some)
                    .map_err(|_| format!("invalid numeric value: {value}"))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    values.resize(count, None);
    values.truncate(count);
    Ok(values)
}

fn save_world(
    path: &Path,
    enchantments: &[Option<(u8, u8)>],
    is_new_world: bool,
) -> Result<(), String> {
    let costs = enchantments
        .iter()
        .map(|value| value.map(|(cost, _)| cost.to_string()).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(",");
    let levels = enchantments
        .iter()
        .map(|value| {
            value
                .map(|(_, level)| level.to_string())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(",");
    let contents = format!("{FILE_HEADER}\n{costs}\n{levels}\n");

    if is_new_world {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| error.to_string())?;
        file.write_all(contents.as_bytes())
            .map_err(|error| error.to_string())
    } else {
        fs::write(path, contents).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_data_round_trips_in_existing_format() {
        let path = std::env::temp_dir().join(format!(
            "mc-help-test-{}-{}.world",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let enchantments = vec![Some((12, 3)), None, Some((4, 1))];

        save_world(&path, &enchantments, true).unwrap();
        let loaded = load_world(&path, enchantments.len()).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(loaded, enchantments);
    }

    #[test]
    fn invalid_cost_or_level_values_are_reported() {
        assert!(parse_values("1,invalid", 2).is_err());
    }

    #[test]
    fn levels_are_formatted_as_roman_numerals() {
        assert_eq!(roman_numeral(1), "I");
        assert_eq!(roman_numeral(4), "IV");
        assert_eq!(roman_numeral(5), "V");
    }

    #[test]
    fn deleting_current_world_removes_file_and_clears_editor() {
        let dir = std::env::temp_dir().join(format!(
            "mc-help-delete-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("world");
        fs::write(&path, "world data").unwrap();

        let mut app = WorldApp::new(dir.clone(), Vec::new());
        app.current_world = Some("world".into());
        app.delete_world("world");

        assert!(!path.exists());
        assert_eq!(app.current_world, None);
        assert!(app.worlds.is_empty());
        fs::remove_dir(dir).unwrap();
    }
}
