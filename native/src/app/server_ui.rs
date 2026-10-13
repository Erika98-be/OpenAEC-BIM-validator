use super::*;
use crate::service::{self, http::Server};
pub(super) struct ApiUi {
    pub server: Option<Server>,
    pub port: u16,
    pub token: String,
    pub page: usize,
    pub tool: usize,
    pub args: String,
    pub result: String,
    pub rx: Option<mpsc::Receiver<String>>,
}
impl Default for ApiUi {
    fn default() -> Self {
        Self {
            server: None,
            port: 19781,
            token: uuid::Uuid::new_v4().to_string(),
            page: 0,
            tool: 0,
            args: "{}".into(),
            result: String::new(),
            rx: None,
        }
    }
}
impl BimApp {
    pub(super) fn server_toggle(&mut self) {
        if let Some(server) = &self.api.server {
            server.stop.cancel();
            self.status = "API/MCP-server stopt…".into();
            return;
        }
        let service = service::Service::new(self.models.clone(), true);
        match Server::start(self.api.port, self.api.token.clone(), service) {
            Ok(server) => {
                self.status = format!("API/MCP luistert op {}", server.address);
                self.api.server = Some(server);
            }
            Err(e) => self.error = Some(e),
        }
    }
    pub(super) fn server_sync(&self) {
        if let Some(server) = &self.api.server {
            server.service.sync(&self.models);
        }
    }
    pub(super) fn server_poll(&mut self) {
        if self.api.server.as_ref().is_some_and(|s| s.finished()) {
            if let Some(mut server) = self.api.server.take() {
                if let Err(e) = server.join() {
                    self.error = Some(e);
                } else {
                    self.status = "API/MCP-server gestopt".into();
                }
            }
        }
        if let Some(service) = self.api.server.as_ref().map(|s| s.service.clone()) {
            let requests: Vec<_> = service.state.lock().unwrap().pending.drain(..).collect();
            for request in requests {
                let revision = service.snapshot().revision;
                let applied = request.revision == revision;
                if applied {
                    self.selected = request.keys.into_iter().collect();
                    if request.focus {
                        self.fit(true);
                    }
                    self.status = "MCP/API-selectie toegepast in viewer".into();
                }
                service.state.lock().unwrap().last_selection = Some(
                    serde_json::json!({"requestId":request.id,"status":if applied{"applied"}else{"stale"}}),
                );
            }
            service.state.lock().unwrap().selection = self.selected.iter().copied().collect();
        }
        if let Some(result) = self.api.rx.as_ref().and_then(|r| r.try_recv().ok()) {
            self.api.result = result;
            self.api.rx = None;
        }
    }
    pub(super) fn server_panel(&mut self, ctx: &egui::Context) {
        if self.tab != 7 {
            return;
        }
        let mut toggle = false;
        let mut run = false;
        egui::SidePanel::right("api-mcp-workspace").default_width(if ctx.available_rect().width()<1200. {380.} else {510.}).width_range(380. ..=800.).resizable(true).show(ctx,|ui|{
            ui.heading("API & MCP-server");
            ui.horizontal(|ui|{for (i,name) in ["Verbinding","Tools testen","Activiteit"].iter().enumerate(){ui.selectable_value(&mut self.api.page,i,*name);}});
            ui.separator();
            egui::ScrollArea::vertical().show(ui,|ui|{
                let running=self.api.server.is_some();
                let stopping=self.api.server.as_ref().is_some_and(|s|s.stop.is_cancelled());
                ui.label(if stopping{"Bezig met stoppen"}else if running{"Actief · alleen deze computer"}else{"Gestopt"});
                if self.api.page==0 {
                    ui.add_enabled_ui(!running,|ui|{ui.horizontal(|ui|{ui.label("Poort");ui.add(egui::DragValue::new(&mut self.api.port).range(0..=65535));});ui.small("Poort 0 kiest automatisch een vrije poort.");});
                    toggle=ui.add_enabled(!stopping,egui::Button::new(if running{"Server stoppen"}else{"Server starten"})).clicked();
                    if let Some(server)=&self.api.server {
                        let url=format!("http://{}",server.address);
                        ui.separator();ui.label("MCP · Streamable HTTP");ui.monospace(format!("{url}/mcp"));
                        ui.label("REST API / OpenAPI");ui.monospace(format!("{url}/api/v1/openapi.json"));
                        ui.label("Toegangstoken");ui.add(egui::TextEdit::singleline(&mut self.api.token).password(true).interactive(false));
                        ui.horizontal_wrapped(|ui|{
                            if ui.button("Token kopiëren").clicked(){ctx.copy_text(self.api.token.clone());}
                            if ui.button("MCP-configuratie kopiëren").clicked(){ctx.copy_text(serde_json::to_string_pretty(&serde_json::json!({"mcpServers":{"openaec":{"url":format!("{url}/mcp"),"headers":{"Authorization":format!("Bearer {}",self.api.token)}}}})).unwrap());}
                            if ui.button("API-adres kopiëren").clicked(){ctx.copy_text(format!("{url}/api/v1"));}
                        });
                        let snapshot=server.service.snapshot();ui.label(format!("{} geladen modellen · revisie {}",snapshot.models.len(),snapshot.revision));
                    } else if ui.button("Nieuw token genereren").clicked(){self.api.token=uuid::Uuid::new_v4().to_string();}
                    ui.separator();ui.label("Modelgegevens en selectie worden gedeeld met verbonden clients. Open modellen via Bestand; de server heeft geen endpoint om willekeurige bestanden te lezen.");
                    ui.label("De server start alleen op jouw commando. Token en serverstatus worden niet opgeslagen in projectbestanden.");
                    ui.label("Exports blijven tijdelijk in het geheugen. Selectieaanroepen verschijnen in de viewer. Andere tools wijzigen je modellen niet.");
                } else if self.api.page==1 {
                    let tools=service::tools();let previous=self.api.tool;
                    egui::ComboBox::from_id_salt("api-tool").width(280.).selected_text(tools[self.api.tool]["name"].as_str().unwrap()).show_ui(ui,|ui|{for (i,tool) in tools.iter().enumerate(){ui.selectable_value(&mut self.api.tool,i,tool["name"].as_str().unwrap());}});
                    if previous!=self.api.tool {self.api.args=match tools[self.api.tool]["name"].as_str().unwrap(){"object_get"=>"{\"model\":0,\"element\":42}","objects_query"=>"{\"limit\":20}","models_diff"=>"{\"before\":0,\"after\":1}","model_export"=>"{\"format\":\"glb\"}","artifact_read"=>"{\"id\":\"plak-artifact-id\"}","view_select"=>"{\"revision\":1,\"objects\":[],\"focus\":false}",_=>"{}"}.into();}
                    if previous!=self.api.tool && tools[self.api.tool]["name"]=="view_select" {
                        let revision=self.api.server.as_ref().map(|server|server.service.snapshot().revision).unwrap_or(0);
                        self.api.args=serde_json::json!({"revision":revision,"objects":[],"focus":false}).to_string();
                    }
                    ui.label(tools[self.api.tool]["description"].as_str().unwrap());
                    ui.collapsing("Argumentschema",|ui|{ui.monospace(serde_json::to_string_pretty(&tools[self.api.tool]["inputSchema"]).unwrap());});
                    ui.label("Argumenten (JSON)");ui.add(egui::TextEdit::multiline(&mut self.api.args).code_editor().desired_rows(6).desired_width(f32::INFINITY));
                    run=ui.add_enabled(running&&!stopping&&self.api.rx.is_none(),egui::Button::new("Uitvoeren via lokale API")).clicked();
                    if self.api.rx.is_some(){ui.spinner();}
                    ui.separator();ui.label("Antwoord");ui.add(egui::TextEdit::multiline(&mut self.api.result).code_editor().desired_rows(14).desired_width(f32::INFINITY));
                }else if let Some(server)=&self.api.server {
                    let state=server.service.state.lock().unwrap();
                    ui.label(format!("{} aanroepen · maximaal 100 bewaard",state.audit.len()));
                    for entry in state.audit.iter().rev(){ui.label(format!("{} · {} · {} ms",if entry.ok{"OK"}else{"Fout"},entry.tool,entry.elapsed_ms));ui.small(&entry.time);ui.separator();}
                } else {ui.label("Start de server om aanroepen te bekijken.");}
            });
        });
        if toggle {
            self.server_toggle();
        }
        if run {
            match serde_json::from_str::<serde_json::Value>(&self.api.args) {
                Err(e) => self.api.result = format!("Ongeldige JSON: {e}"),
                Ok(args) => {
                    if let Some(server) = &self.api.server {
                        let url = format!(
                            "http://{}/api/v1/tools/{}",
                            server.address,
                            service::tools()[self.api.tool]["name"].as_str().unwrap()
                        );
                        let token = self.api.token.clone();
                        let (tx, rx) = mpsc::channel();
                        self.api.rx = Some(rx);
                        let context = ctx.clone();
                        std::thread::spawn(move || {
                            let result = (|| {
                                let client = reqwest::blocking::Client::builder()
                                    .timeout(std::time::Duration::from_secs(120))
                                    .no_proxy()
                                    .build()
                                    .map_err(|e| e.to_string())?;
                                let response = client
                                    .post(url)
                                    .bearer_auth(token)
                                    .json(&args)
                                    .send()
                                    .map_err(|e| e.to_string())?;
                                let status = response.status();
                                let value: serde_json::Value =
                                    response.json().map_err(|e| e.to_string())?;
                                Ok::<_, String>(format!(
                                    "HTTP {status}\n{}",
                                    serde_json::to_string_pretty(&value).unwrap()
                                ))
                            })();
                            let _ =
                                tx.send(result.unwrap_or_else(|e| format!("Aanroep mislukt: {e}")));
                            context.request_repaint();
                        });
                    }
                }
            }
        }
    }
}
