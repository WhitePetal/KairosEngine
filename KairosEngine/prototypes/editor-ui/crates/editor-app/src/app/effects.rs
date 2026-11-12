//! Phase 4: execute effects against host resources (files, GPU viewport, window)
//! and route completions back through `App::complete`.
//!
//! This is the only place where the editor shell touches IO or the wgpu viewport
//! host; reducers stay pure and are unaware of these implementations.

use editor_core_prototype::{Effect, EffectOutcome, SceneObjectId, ViewportId};
use editor_viewport_prototype::CameraInput as GpuCameraInput;

use super::EditorApp;

impl EditorApp {
    /// Execute one effect. Returning effects is how completions feed the
    /// reducer: `App::complete` returns the effects produced by the completion
    /// action, and the pump loop runs them in the same frame.
    pub(super) fn apply_effect(&mut self, ctx: &egui::Context, effect: Effect) -> Vec<Effect> {
        match effect {
            Effect::SaveDocument {
                ticket,
                document,
                bytes,
            } => {
                let dir = std::env::temp_dir().join("kairos_editor_ui_prototype");
                let path = dir.join(format!("document_{}.bin", document.0));
                let result =
                    std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, &bytes));
                match result {
                    Ok(()) => {
                        self.host.log(format!(
                            "SaveDocument -> {} ({} bytes), completing async ticket",
                            path.display(),
                            bytes.len()
                        ));
                        self.core.complete(ticket, EffectOutcome::DocumentSaved)
                    }
                    Err(error) => {
                        self.host.log(format!("SaveDocument failed: {error}"));
                        self.core.complete(
                            ticket,
                            EffectOutcome::DocumentSaveFailed {
                                error: error.to_string(),
                            },
                        )
                    }
                }
            }
            Effect::LoadAsset {
                ticket,
                asset,
                path,
            } => {
                self.host.log(format!(
                    "LoadAsset {{ asset: {:?}, path: {path} }} -> completed synchronously",
                    asset
                ));
                self.core.complete(ticket, EffectOutcome::AssetLoaded)
            }
            Effect::SpawnPreview { ticket, asset } => {
                self.host.log(format!(
                    "SpawnPreview {{ asset: {:?} }} -> completed synchronously",
                    asset
                ));
                self.core.complete(ticket, EffectOutcome::PreviewSpawned)
            }
            Effect::FocusPanel { panel } => {
                self.host.log(format!("FocusPanel {panel:?}"));
                Vec::new()
            }
            Effect::ShowNativeDialog { title, message } => {
                self.host
                    .log(format!("ShowNativeDialog [{title}] {message}"));
                Vec::new()
            }
            Effect::PersistProject => {
                self.host
                    .log("PersistProject (host file not written by the prototype)");
                Vec::new()
            }
            Effect::PersistLayout { layout } => {
                self.host.log(format!(
                    "PersistLayout (host file not written): {} panels open",
                    layout.open_panels.len()
                ));
                Vec::new()
            }
            Effect::Quit => {
                self.host.log("Quit -> closing viewport");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                Vec::new()
            }

            // --- viewport / GPU ---------------------------------------
            Effect::CreateViewport {
                ticket,
                viewport,
                width,
                height,
            } => self.create_viewport(ticket, viewport, width, height),
            Effect::SyncViewportSize {
                viewport,
                width,
                height,
            } => {
                if let Some(host) = self.host.viewport.as_mut()
                    && let Err(error) = host.resize(viewport.0, width, height)
                {
                    self.host
                        .log(format!("resize viewport {viewport:?} failed: {error}"));
                }
                Vec::new()
            }
            Effect::DestroyViewport { viewport } => {
                if let Some(host) = self.host.viewport.as_mut() {
                    host.destroy(viewport.0);
                }
                self.host.log(format!("DestroyViewport {viewport:?}"));
                Vec::new()
            }
            Effect::ViewportCameraInput { viewport, input } => {
                if let Some(host) = self.host.viewport.as_mut() {
                    host.apply_camera_input(
                        viewport.0,
                        GpuCameraInput {
                            orbit: input.orbit,
                            pan: input.pan,
                            zoom: input.zoom,
                        },
                    );
                }
                // The intent is applied after this frame's render; make sure the
                // next frame renders the new camera pose.
                ctx.request_repaint();
                Vec::new()
            }
            Effect::PickViewport {
                ticket,
                viewport,
                ndc,
            } => {
                let object = self
                    .host
                    .viewport
                    .as_ref()
                    .and_then(|host| host.pick(viewport.0, ndc));
                match object {
                    Some(object) => self.host.log(format!("picked scene object {object}")),
                    None => self.host.log("picked nothing (cleared scene selection)"),
                }
                self.core.complete(
                    ticket,
                    EffectOutcome::SceneObjectPicked {
                        object: object.map(SceneObjectId),
                    },
                )
            }
        }
    }

    fn create_viewport(
        &mut self,
        ticket: editor_core_prototype::EffectTicket,
        viewport: ViewportId,
        width: u32,
        height: u32,
    ) -> Vec<Effect> {
        let Some(host) = self.host.viewport.as_mut() else {
            self.host
                .log("CreateViewport failed: no wgpu render state available");
            return self.core.complete(
                ticket,
                EffectOutcome::ViewportBindFailed {
                    error: "no wgpu render state available".to_string(),
                },
            );
        };
        match host.bind(viewport.0, width, height) {
            Ok(_) => {
                self.host.log(format!(
                    "CreateViewport {viewport:?} at {width}x{height} -> bound"
                ));
                self.core.complete(ticket, EffectOutcome::ViewportBound)
            }
            Err(error) => {
                self.host
                    .log(format!("CreateViewport {viewport:?} failed: {error}"));
                self.core.complete(
                    ticket,
                    EffectOutcome::ViewportBindFailed {
                        error: error.to_string(),
                    },
                )
            }
        }
    }
}
