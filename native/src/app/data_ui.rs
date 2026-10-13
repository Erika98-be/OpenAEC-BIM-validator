use super::*;
impl BimApp {
    pub(super) fn data_panel(&mut self, ctx: &egui::Context) {
        if self.tab != 5 {
            return;
        }
        egui::TopBottomPanel::bottom("data-workspace")
            .resizable(true)
            .default_height(310.)
            .height_range(160. ..=650.)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Modeldata");
                    ui.selectable_value(&mut self.data_mode, 0, "Objecten & hoeveelheden");
                    ui.selectable_value(&mut self.data_mode, 1, "Versies vergelijken");
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button("3D export · GLB…").clicked() {
                        self.command("export-glb", ctx);
                    }
                    ui.label("Snedehoogte");
                    ui.add(
                        egui::DragValue::new(&mut self.export_height)
                            .speed(0.1)
                            .suffix(" m"),
                    );
                    if ui.button("2D snede · SVG…").clicked() {
                        self.command("export-svg", ctx);
                    }
                });
                ui.separator();
                if self.data_mode == 0 {
                    self.schedule_ui(ui);
                } else {
                    self.diff_ui(ui);
                }
            });
    }
    fn schedule_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.query.text)
                    .desired_width(180.)
                    .hint_text("Naam, type of GlobalId"),
            );
            let classes: std::collections::BTreeSet<_> = self
                .models
                .iter()
                .flat_map(|m| m.elements.values().map(|e| e.class.clone()))
                .collect();
            egui::ComboBox::from_id_salt("data-type")
                .selected_text(if self.query.class.is_empty() {
                    "Alle IFC-types"
                } else {
                    &self.query.class
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.query.class, String::new(), "Alle IFC-types");
                    for class in classes {
                        ui.selectable_value(&mut self.query.class, class.clone(), class);
                    }
                });
            egui::ComboBox::from_id_salt("data-column")
                .width(220.)
                .selected_text(&self.data_column)
                .show_ui(ui, |ui| {
                    for c in crate::data::columns(&self.models) {
                        ui.selectable_value(&mut self.data_column, c.clone(), c);
                    }
                });
            if ui.button("Filter wissen").clicked() {
                self.query = Default::default();
                self.data_filter = false;
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Kolomfilter");
            ui.checkbox(&mut self.data_filter, "Actief");
            let ops = [
                "bevat",
                "is gelijk aan",
                "ontbreekt",
                "is ingevuld",
                ">",
                "<",
            ];
            egui::ComboBox::from_id_salt("data-operator")
                .selected_text(ops[self.query.operator])
                .show_ui(ui, |ui| {
                    for (i, label) in ops.iter().enumerate() {
                        ui.selectable_value(&mut self.query.operator, i, *label);
                    }
                });
            ui.add(
                egui::TextEdit::singleline(&mut self.query.value)
                    .desired_width(150.)
                    .hint_text("Waarde"),
            );
        });
        self.query.property = if self.data_filter {
            self.data_column.clone()
        } else {
            String::new()
        };
        let rows = self.query.rows(&self.models);
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} objecten",rows.len()));
            if ui.button("Selecteer resultaten").clicked(){self.selected=rows.iter().copied().collect();}
            if ui.button("Isoleer resultaten").clicked(){self.selected=rows.iter().copied().collect();self.isolate();}
            if ui.button("CSV exporteren…").clicked(){if let Some(path)=rfd::FileDialog::new().add_filter("CSV",&["csv"]).set_file_name("modeldata.csv").save_file(){match std::fs::write(path,crate::data::csv(&self.models,&rows,&self.data_column)){Ok(())=>self.status=format!("{} tabelregels geëxporteerd",rows.len()),Err(e)=>self.error=Some(e.to_string())}}}
            let values:Vec<f64>=rows.iter().filter_map(|&(mi,id)|crate::data::property(&self.models[mi].elements[&id],&self.data_column).parse::<f64>().ok().filter(|n|n.is_finite())).collect();
            if !values.is_empty(){ui.label(format!("Numeriek: {} · som {:.3}",values.len(),values.iter().sum::<f64>())).on_hover_text("Som van bronwaarden; controleer dat modellen dezelfde eenheid gebruiken. Geen eenheidsconversie.");}
        });
        ui.separator();
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_rows(ui, 24., rows.len(), |ui, range| {
                for row in &rows[range] {
                    let e = &self.models[row.0].elements[&row.1];
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                self.selected.contains(row),
                                format!("{} · #{}", self.models[row.0].name, e.id),
                            )
                            .clicked()
                        {
                            self.selected = HashSet::from([*row]);
                        }
                        ui.label(&e.class);
                        ui.label(&e.name);
                        ui.monospace(crate::data::property(e, &self.data_column));
                    });
                }
            });
    }
    fn diff_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (label, index) in [
                ("Oude versie", &mut self.diff_before),
                ("Nieuwe versie", &mut self.diff_after),
            ] {
                egui::ComboBox::from_id_salt(label)
                    .selected_text(
                        self.models
                            .get(*index)
                            .map(|m| m.name.as_str())
                            .unwrap_or(label),
                    )
                    .show_ui(ui, |ui| {
                        for (i, m) in self.models.iter().enumerate() {
                            if ui.selectable_value(index, i, &m.name).changed() {
                                self.differences.clear();
                            }
                        }
                    });
            }
            if ui
                .add_enabled(self.models.len() > 1, egui::Button::new("Vergelijken"))
                .clicked()
            {
                match crate::data::compare(&self.models, self.diff_before, self.diff_after) {
                    Ok(rows) => self.differences = rows,
                    Err(e) => self.error = Some(e),
                }
            }
            if ui
                .add_enabled(
                    !self.differences.is_empty(),
                    egui::Button::new("JSON exporteren…"),
                )
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_file_name("modelverschillen.json")
                    .save_file()
                {
                    match serde_json::to_vec_pretty(&self.differences)
                        .map_err(|e| e.to_string())
                        .and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()))
                    {
                        Ok(()) => self.status = "Modelvergelijking geëxporteerd".into(),
                        Err(e) => self.error = Some(e),
                    }
                }
            }
        });
        ui.small("Koppeling op GlobalId. Objecten zonder GlobalId worden overgeslagen. Meshvergelijking op een 1 mm raster; gewijzigde triangulatie telt als verschil.");
        ui.label(format!("{} verschillen", self.differences.len()));
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_rows(ui, 26., self.differences.len(), |ui, range| {
                for diff in &self.differences[range] {
                    ui.horizontal(|ui| {
                        let key = diff.after.or(diff.before);
                        if ui
                            .selectable_label(
                                key.is_some_and(|k| self.selected.contains(&k)),
                                format!("{} · {}", diff.status, diff.name),
                            )
                            .clicked()
                        {
                            self.selected = diff.before.into_iter().chain(diff.after).collect();
                        }
                        ui.label(diff.changes.join(", "));
                        ui.monospace(&diff.guid);
                    });
                }
            });
    }
}
