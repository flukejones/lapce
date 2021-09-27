use crate::{
    command::LapceUICommand,
    command::LAPCE_UI_COMMAND,
    config::LapceTheme,
    data::{LapceTabData, LapceTabLens, LapceWindowData},
    editor::EditorUIState,
    explorer::{FileExplorer, FileExplorerState},
    panel::{LapcePanel, PanelPosition, PanelProperty},
    state::{LapceWorkspace, LapceWorkspaceType},
    tab::LapceTabNew,
    theme::OldLapceTheme,
};
use druid::{
    kurbo::Line,
    piet::{Text, TextLayout, TextLayoutBuilder},
    theme,
    widget::IdentityWrapper,
    widget::WidgetExt,
    BoxConstraints, Command, Env, Event, EventCtx, FontDescriptor, FontFamily,
    LayoutCtx, Lens, LifeCycle, LifeCycleCtx, PaintCtx, Point, Rect, RenderContext,
    Size, Target, UpdateCtx, Widget, WidgetId, WidgetPod, WindowId,
};
use parking_lot::Mutex;
use std::{collections::HashMap, ops::Index, sync::Arc};

pub struct LapceWindowNew {
    pub tabs: Vec<WidgetPod<LapceWindowData, Box<dyn Widget<LapceWindowData>>>>,
}

impl LapceWindowNew {
    pub fn new(data: &LapceWindowData) -> Self {
        let tabs = data
            .tabs
            .iter()
            .map(|(tab_id, data)| {
                let tab = LapceTabNew::new(data);
                let tab = tab.lens(LapceTabLens(*tab_id));
                WidgetPod::new(tab.boxed())
            })
            .collect();
        Self { tabs }
    }

    pub fn new_tab(
        &mut self,
        ctx: &mut EventCtx,
        data: &mut LapceWindowData,
        workspace: Option<LapceWorkspace>,
        replace_current: bool,
    ) {
        let tab_id = WidgetId::next();
        let mut tab_data = LapceTabData::new(
            tab_id,
            data.keypress.clone(),
            data.theme.clone(),
            Some(ctx.get_external_handle()),
        );
        if let Some(workspace) = workspace {
            tab_data.set_workspace(workspace, ctx.get_external_handle());
        }
        let tab = LapceTabNew::new(&tab_data).lens(LapceTabLens(tab_id));
        data.tabs.insert(tab_id, tab_data);
        if replace_current {
            self.tabs[data.active] = WidgetPod::new(tab.boxed());
            if let Some(tab) = data.tabs.remove(&data.active_id) {
                tab.proxy.stop();
            }
            data.active_id = tab_id;
        } else {
            self.tabs
                .insert(data.active + 1, WidgetPod::new(tab.boxed()));
            data.active = data.active + 1;
            data.active_id = tab_id;
        }
        ctx.submit_command(Command::new(
            LAPCE_UI_COMMAND,
            LapceUICommand::FocusTab,
            Target::Auto,
        ));
        ctx.children_changed();
        ctx.set_handled();
        ctx.request_layout();
        return;
    }

    pub fn close_tab(&mut self, ctx: &mut EventCtx, data: &mut LapceWindowData) {
        if data.tabs.len() == 1 {
            return;
        }

        self.tabs.remove(data.active);
        if let Some(tab) = data.tabs.remove(&data.active_id) {
            tab.proxy.stop();
        }

        if data.active >= self.tabs.len() {
            data.active = self.tabs.len() - 1;
        }
        data.active_id = self.tabs[data.active].id();

        ctx.submit_command(Command::new(
            LAPCE_UI_COMMAND,
            LapceUICommand::FocusTab,
            Target::Auto,
        ));
        ctx.children_changed();
        ctx.set_handled();
        ctx.request_layout();
    }
}

impl Widget<LapceWindowData> for LapceWindowNew {
    fn event(
        &mut self,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut LapceWindowData,
        env: &Env,
    ) {
        match event {
            Event::Command(cmd) if cmd.is(LAPCE_UI_COMMAND) => {
                let command = cmd.get_unchecked(LAPCE_UI_COMMAND);
                match command {
                    LapceUICommand::SetWorkspace(workspace) => {
                        self.new_tab(ctx, data, Some(workspace.clone()), true);
                        return;
                    }
                    LapceUICommand::NewTab => {
                        self.new_tab(ctx, data, None, false);
                        return;
                    }
                    LapceUICommand::CloseTab => {
                        self.close_tab(ctx, data);
                        return;
                    }
                    LapceUICommand::NextTab => {
                        let new_index = if data.active >= self.tabs.len() - 1 {
                            0
                        } else {
                            data.active + 1
                        };
                        data.active = new_index;
                        data.active_id = self.tabs[new_index].id();
                        ctx.submit_command(Command::new(
                            LAPCE_UI_COMMAND,
                            LapceUICommand::FocusTab,
                            Target::Auto,
                        ));
                        ctx.request_layout();
                        ctx.set_handled();
                    }
                    LapceUICommand::PreviousTab => {
                        let new_index = if data.active == 0 {
                            self.tabs.len() - 1
                        } else {
                            data.active - 1
                        };
                        data.active = new_index;
                        data.active_id = self.tabs[new_index].id();
                        ctx.submit_command(Command::new(
                            LAPCE_UI_COMMAND,
                            LapceUICommand::FocusTab,
                            Target::Auto,
                        ));
                        ctx.request_layout();
                        ctx.set_handled();
                    }
                    _ => (),
                }
            }
            _ => (),
        }
        self.tabs[data.active].event(ctx, event, data, env);
    }

    fn lifecycle(
        &mut self,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &LapceWindowData,
        env: &Env,
    ) {
        for tab in self.tabs.iter_mut() {
            tab.lifecycle(ctx, event, data, env);
        }
    }

    fn update(
        &mut self,
        ctx: &mut druid::UpdateCtx,
        old_data: &LapceWindowData,
        data: &LapceWindowData,
        env: &Env,
    ) {
        if old_data.active != data.active {
            ctx.request_layout();
        }
        let old_tab = old_data.tabs.get(&old_data.active_id).unwrap();
        let tab = data.tabs.get(&data.active_id).unwrap();
        if old_tab.workspace != tab.workspace {
            ctx.request_layout();
        }
        let start = std::time::SystemTime::now();
        self.tabs[data.active].update(ctx, data, env);
        let end = std::time::SystemTime::now();
        let duration = end.duration_since(start).unwrap().as_micros();
        // println!("update took {}", duration);
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &LapceWindowData,
        env: &Env,
    ) -> Size {
        let self_size = bc.max();

        let (tab_size, tab_origin) = if self.tabs.len() > 1 {
            let tab_height = 25.0;
            let tab_size = Size::new(self_size.width, self_size.height - tab_height);
            let tab_origin = Point::new(0.0, tab_height);
            (tab_size, tab_origin)
        } else {
            (self_size.clone(), Point::ZERO)
        };

        let start = std::time::SystemTime::now();
        let tab = &mut self.tabs[data.active];
        let bc = BoxConstraints::tight(tab_size);
        tab.layout(ctx, &bc, data, env);
        tab.set_origin(ctx, data, env, tab_origin);
        let end = std::time::SystemTime::now();
        let duration = end.duration_since(start).unwrap().as_micros();
        // println!("layout took {}", duration);
        ctx.submit_command(Command::new(
            LAPCE_UI_COMMAND,
            LapceUICommand::UpdateWindowOrigin,
            Target::Widget(data.active_id),
        ));

        self_size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &LapceWindowData, env: &Env) {
        let tab_height = 25.0;
        let size = ctx.size();
        if self.tabs.len() > 1 {
            ctx.fill(
                Size::new(size.width, tab_height).to_rect(),
                data.config
                    .get_color_unchecked(LapceTheme::LAPCE_INACTIVE_TAB),
            );
            let color = env.get(theme::BORDER_LIGHT);
            let num = self.tabs.len();
            let section = size.width / num as f64;
            for (i, tab) in self.tabs.iter().enumerate() {
                let tab_id = tab.id();
                if i == data.active {
                    ctx.fill(
                        Rect::ZERO
                            .with_origin(Point::new(section * i as f64, 0.0))
                            .with_size(Size::new(section, tab_height)),
                        data.config
                            .get_color_unchecked(LapceTheme::LAPCE_ACTIVE_TAB),
                    );
                }
                let tab = data.tabs.get(&tab_id).unwrap();
                let dir = tab
                    .workspace
                    .as_ref()
                    .map(|w| {
                        let dir = w.path.file_name().unwrap().to_str().unwrap();
                        let dir = match &w.kind {
                            LapceWorkspaceType::Local => dir.to_string(),
                            LapceWorkspaceType::RemoteSSH(user, host) => {
                                format!("{} [{}@{}]", dir, user, host)
                            }
                        };
                        dir
                    })
                    .unwrap_or("Lapce".to_string());
                let text_layout = ctx
                    .text()
                    .new_text_layout(dir)
                    .font(FontFamily::SYSTEM_UI, 13.0)
                    .text_color(
                        tab.config
                            .get_color_unchecked(LapceTheme::EDITOR_FOREGROUND)
                            .clone(),
                    )
                    .build()
                    .unwrap();

                let text_width = text_layout.size().width;
                let x = (section - text_width) / 2.0 + section * i as f64;
                ctx.draw_text(&text_layout, Point::new(x, 3.0));
            }
            for i in 1..num {
                let line = Line::new(
                    Point::new(i as f64 * section, 0.0),
                    Point::new(i as f64 * section, tab_height),
                );
                ctx.stroke(line, &color, 1.0);
            }
        }
        self.tabs[data.active].paint(ctx, data, env);
        if self.tabs.len() > 1 {
            let num = self.tabs.len();
            let section = size.width / num as f64;

            ctx.fill(
                Rect::ZERO
                    .with_origin(Point::new(section * data.active as f64, 0.0))
                    .with_size(Size::new(section, tab_height)),
                data.config
                    .get_color_unchecked(LapceTheme::LAPCE_ACTIVE_TAB),
            );

            let tab = data.tabs.get(&self.tabs[data.active].id()).unwrap();
            let dir = tab
                .workspace
                .as_ref()
                .map(|w| {
                    let dir = w.path.file_name().unwrap().to_str().unwrap();
                    let dir = match &w.kind {
                        LapceWorkspaceType::Local => dir.to_string(),
                        LapceWorkspaceType::RemoteSSH(user, host) => {
                            format!("{} [{}@{}]", dir, user, host)
                        }
                    };
                    dir
                })
                .unwrap_or("Lapce".to_string());
            let text_layout = ctx
                .text()
                .new_text_layout(dir)
                .font(FontFamily::SYSTEM_UI, 13.0)
                .text_color(
                    tab.config
                        .get_color_unchecked(LapceTheme::EDITOR_FOREGROUND)
                        .clone(),
                )
                .build()
                .unwrap();
            let text_width = text_layout.size().width;
            let x = (section - text_width) / 2.0 + section * data.active as f64;
            ctx.draw_text(&text_layout, Point::new(x, 3.0));

            let line = Line::new(
                Point::new(0.0, tab_height - 0.5),
                Point::new(size.width, tab_height - 0.5),
            );
            let color = env.get(theme::BORDER_LIGHT);
            ctx.stroke(line, &color, 1.0);
        }
    }
}
