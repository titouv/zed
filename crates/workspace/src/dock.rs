use crate::persistence::model::DockData;
use crate::{status_bar::StatusItemView, Workspace};
use crate::{DraggedDock, Event, Pane};
use client::proto;
use gpui::{
    deferred, div, px, Action, AnyView, AppContext, Axis, Corner, Entity, EntityId, EventEmitter,
    FocusHandle, FocusableView, IntoElement, KeyContext, Model, ModelContext, MouseButton,
    MouseDownEvent, MouseUpEvent, ParentElement, Render, SharedString, StyleRefinement, Styled,
    Subscription, VisualContext, WeakModel, Window,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings::SettingsStore;
use std::sync::Arc;
use ui::{h_flex, ContextMenu, IconButton, Tooltip};
use ui::{prelude::*, right_click_menu};

pub(crate) const RESIZE_HANDLE_SIZE: Pixels = Pixels(6.);

pub enum PanelEvent {
    ZoomIn,
    ZoomOut,
    Activate,
    Close,
}

pub use proto::PanelId;

pub trait Panel: FocusableView + EventEmitter<PanelEvent> + Render + Sized {
    fn persistent_name() -> &'static str;
    fn position(&self, window: &Window, cx: &AppContext) -> DockPosition;
    fn position_is_valid(&self, position: DockPosition) -> bool;
    fn set_position(
        &mut self,
        position: DockPosition,
        window: &mut Window,
        cx: &mut ModelContext<Self>,
    );
    fn size(&self, window: &Window, cx: &AppContext) -> Pixels;
    fn set_size(&mut self, size: Option<Pixels>, window: &mut Window, cx: &mut ModelContext<Self>);
    fn icon(&self, window: &Window, cx: &AppContext) -> Option<ui::IconName>;
    fn icon_tooltip(&self, window: &Window, cx: &AppContext) -> Option<&'static str>;
    fn toggle_action(&self) -> Box<dyn Action>;
    fn icon_label(&self, _window: &Window, _: &AppContext) -> Option<String> {
        None
    }
    fn is_zoomed(&self, _window: &Window, _cx: &AppContext) -> bool {
        false
    }
    fn starts_open(&self, _window: &Window, _cx: &AppContext) -> bool {
        false
    }
    fn set_zoomed(&mut self, _zoomed: bool, _window: &mut Window, _cx: &mut ModelContext<Self>) {}
    fn set_active(&mut self, _active: bool, _window: &mut Window, _cx: &mut ModelContext<Self>) {}
    fn pane(&self) -> Option<Model<Pane>> {
        None
    }
    fn remote_id() -> Option<proto::PanelId> {
        None
    }
    fn activation_priority(&self) -> u32;
}

pub trait PanelHandle: Send + Sync {
    fn panel_id(&self) -> EntityId;
    fn persistent_name(&self) -> &'static str;
    fn position(&self, window: &Window, cx: &AppContext) -> DockPosition;
    fn position_is_valid(&self, position: DockPosition, window: &Window, cx: &AppContext) -> bool;
    fn set_position(&self, position: DockPosition, window: &mut Window, cx: &mut AppContext);
    fn is_zoomed(&self, window: &Window, cx: &AppContext) -> bool;
    fn set_zoomed(&self, zoomed: bool, window: &mut Window, cx: &mut AppContext);
    fn set_active(&self, active: bool, window: &mut Window, cx: &mut AppContext);
    fn remote_id(&self) -> Option<proto::PanelId>;
    fn pane(&self, window: &Window, cx: &AppContext) -> Option<Model<Pane>>;
    fn size(&self, window: &Window, cx: &AppContext) -> Pixels;
    fn set_size(&self, size: Option<Pixels>, window: &mut Window, cx: &mut AppContext);
    fn icon(&self, window: &Window, cx: &AppContext) -> Option<ui::IconName>;
    fn icon_tooltip(&self, window: &Window, cx: &AppContext) -> Option<&'static str>;
    fn toggle_action(&self, window: &Window, cx: &AppContext) -> Box<dyn Action>;
    fn icon_label(&self, window: &Window, cx: &AppContext) -> Option<String>;
    fn panel_focus_handle(&self, cx: &AppContext) -> FocusHandle;
    fn to_any(&self) -> AnyView;
    fn activation_priority(&self, cx: &AppContext) -> u32;
}

impl<T> PanelHandle for Model<T>
where
    T: Panel,
{
    fn panel_id(&self) -> EntityId {
        Entity::entity_id(self)
    }

    fn persistent_name(&self) -> &'static str {
        T::persistent_name()
    }

    fn position(&self, window: &Window, cx: &AppContext) -> DockPosition {
        self.read(cx).position(window, cx)
    }

    fn position_is_valid(&self, position: DockPosition, window: &Window, cx: &AppContext) -> bool {
        self.read(cx).position_is_valid(position)
    }

    fn set_position(&self, position: DockPosition, window: &mut Window, cx: &mut AppContext) {
        self.update(cx, |this, cx| this.set_position(position, window, cx))
    }

    fn is_zoomed(&self, window: &Window, cx: &AppContext) -> bool {
        self.read(cx).is_zoomed(window, cx)
    }

    fn set_zoomed(&self, zoomed: bool, window: &mut Window, cx: &mut AppContext) {
        self.update(cx, |this, cx| this.set_zoomed(zoomed, window, cx))
    }

    fn set_active(&self, active: bool, window: &mut Window, cx: &mut AppContext) {
        self.update(cx, |this, cx| this.set_active(active, window, cx))
    }

    fn pane(&self, window: &Window, cx: &AppContext) -> Option<Model<Pane>> {
        self.read(cx).pane()
    }

    fn remote_id(&self) -> Option<PanelId> {
        T::remote_id()
    }

    fn size(&self, window: &Window, cx: &AppContext) -> Pixels {
        self.read(cx).size(window, cx)
    }

    fn set_size(&self, size: Option<Pixels>, window: &mut Window, cx: &mut AppContext) {
        self.update(cx, |this, cx| this.set_size(size, window, cx))
    }

    fn icon(&self, window: &Window, cx: &AppContext) -> Option<ui::IconName> {
        self.read(cx).icon(window, cx)
    }

    fn icon_tooltip(&self, window: &Window, cx: &AppContext) -> Option<&'static str> {
        self.read(cx).icon_tooltip(window, cx)
    }

    fn toggle_action(&self, window: &Window, cx: &AppContext) -> Box<dyn Action> {
        self.read(cx).toggle_action()
    }

    fn icon_label(&self, window: &Window, cx: &AppContext) -> Option<String> {
        self.read(cx).icon_label(window, cx)
    }

    fn to_any(&self) -> AnyView {
        self.clone().into()
    }

    fn panel_focus_handle(&self, cx: &AppContext) -> FocusHandle {
        self.read(cx).focus_handle(cx).clone()
    }

    fn activation_priority(&self, cx: &AppContext) -> u32 {
        self.read(cx).activation_priority()
    }
}

impl From<&dyn PanelHandle> for AnyView {
    fn from(val: &dyn PanelHandle) -> Self {
        val.to_any()
    }
}

/// A container with a fixed [`DockPosition`] adjacent to a certain widown edge.
/// Can contain multiple panels and show/hide itself with all contents.
pub struct Dock {
    position: DockPosition,
    panel_entries: Vec<PanelEntry>,
    is_open: bool,
    active_panel_index: Option<usize>,
    focus_handle: FocusHandle,
    pub(crate) serialized_dock: Option<DockData>,
    resizeable: bool,
    _subscriptions: [Subscription; 2],
}

impl FocusableView for Dock {
    fn focus_handle(&self, _: &AppContext) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DockPosition {
    Left,
    Bottom,
    Right,
}

impl DockPosition {
    fn label(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Bottom => "bottom",
            Self::Right => "right",
        }
    }

    pub fn axis(&self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Horizontal,
            Self::Bottom => Axis::Vertical,
        }
    }
}

struct PanelEntry {
    panel: Arc<dyn PanelHandle>,
    _subscriptions: [Subscription; 3],
}

pub struct PanelButtons {
    dock: Model<Dock>,
}

impl Dock {
    pub fn new(
        position: DockPosition,
        window: &mut Window,
        cx: &mut ModelContext<Workspace>,
    ) -> Model<Self> {
        let focus_handle = cx.focus_handle();
        let workspace = cx.view().clone();
        let dock = window.new_view(cx, |window: &mut Window, cx: &mut ModelContext<Self>| {
            let focus_subscription = cx.on_focus(&focus_handle, window, |dock, window, cx| {
                if let Some(active_entry) = dock.active_panel_entry() {
                    active_entry.panel.panel_focus_handle(cx).focus(window)
                }
            });
            let zoom_subscription = cx.subscribe_in(
                &workspace,
                window,
                |dock, workspace, e: &Event, window, cx| {
                    if matches!(e, Event::ZoomChanged) {
                        let is_zoomed = workspace.read(cx).zoomed.is_some();
                        dock.resizeable = !is_zoomed;
                    }
                },
            );
            Self {
                position,
                panel_entries: Default::default(),
                active_panel_index: None,
                is_open: false,
                focus_handle: focus_handle.clone(),
                _subscriptions: [focus_subscription, zoom_subscription],
                serialized_dock: None,
                resizeable: true,
            }
        });

        cx.on_focus_in(&focus_handle, window, {
            let dock = dock.downgrade();
            move |workspace, window, cx| {
                let Some(dock) = dock.upgrade() else {
                    return;
                };
                let Some(panel) = dock.read(cx).active_panel() else {
                    return;
                };
                if panel.is_zoomed(window, cx) {
                    workspace.zoomed = Some(panel.to_any().downgrade());
                    workspace.zoomed_position = Some(position);
                } else {
                    workspace.zoomed = None;
                    workspace.zoomed_position = None;
                }
                cx.emit(Event::ZoomChanged);
                workspace.dismiss_zoomed_items_to_reveal(Some(position), window, cx);
                workspace.update_active_view_for_followers(window, cx)
            }
        })
        .detach();

        cx.observe_in(&dock, window, move |workspace, dock, window, cx| {
            if dock.read(cx).is_open() {
                if let Some(panel) = dock.read(cx).active_panel() {
                    if panel.is_zoomed(window, cx) {
                        workspace.zoomed = Some(panel.to_any().downgrade());
                        workspace.zoomed_position = Some(position);
                        cx.emit(Event::ZoomChanged);
                        return;
                    }
                }
            }
            if workspace.zoomed_position == Some(position) {
                workspace.zoomed = None;
                workspace.zoomed_position = None;
                cx.emit(Event::ZoomChanged);
            }
        })
        .detach();

        dock
    }

    pub fn position(&self) -> DockPosition {
        self.position
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn panel<T: Panel>(&self) -> Option<Model<T>> {
        self.panel_entries
            .iter()
            .find_map(|entry| entry.panel.to_any().clone().downcast().ok())
    }

    pub fn panel_index_for_type<T: Panel>(&self) -> Option<usize> {
        self.panel_entries
            .iter()
            .position(|entry| entry.panel.to_any().downcast::<T>().is_ok())
    }

    pub fn panel_index_for_persistent_name(
        &self,
        ui_name: &str,
        _cx: &AppContext,
    ) -> Option<usize> {
        self.panel_entries
            .iter()
            .position(|entry| entry.panel.persistent_name() == ui_name)
    }

    pub fn panel_index_for_proto_id(&self, panel_id: PanelId) -> Option<usize> {
        self.panel_entries
            .iter()
            .position(|entry| entry.panel.remote_id() == Some(panel_id))
    }

    fn active_panel_entry(&self) -> Option<&PanelEntry> {
        self.active_panel_index
            .and_then(|index| self.panel_entries.get(index))
    }

    pub(crate) fn set_open(
        &mut self,
        open: bool,
        window: &mut Window,
        cx: &mut ModelContext<Self>,
    ) {
        if open != self.is_open {
            self.is_open = open;
            if let Some(active_panel) = self.active_panel_entry() {
                active_panel.panel.set_active(open, window, cx);
            }

            cx.notify();
        }
    }

    pub fn set_panel_zoomed(
        &mut self,
        panel: &AnyView,
        zoomed: bool,
        window: &mut Window,
        cx: &mut ModelContext<Self>,
    ) {
        for entry in &mut self.panel_entries {
            if entry.panel.panel_id() == panel.entity_id() {
                if zoomed != entry.panel.is_zoomed(window, cx) {
                    entry.panel.set_zoomed(zoomed, window, cx);
                }
            } else if entry.panel.is_zoomed(window, cx) {
                entry.panel.set_zoomed(false, window, cx);
            }
        }

        cx.notify();
    }

    pub fn zoom_out(&mut self, window: &mut Window, cx: &mut ModelContext<Self>) {
        for entry in &mut self.panel_entries {
            if entry.panel.is_zoomed(window, cx) {
                entry.panel.set_zoomed(false, window, cx);
            }
        }
    }

    pub(crate) fn add_panel<T: Panel>(
        &mut self,
        panel: Model<T>,
        workspace: WeakModel<Workspace>,
        window: &mut Window,
        cx: &mut ModelContext<Self>,
    ) -> usize {
        let subscriptions = [
            cx.observe_in(&panel, window, |_, _, window, cx| cx.notify()),
            cx.observe_global_in::<SettingsStore>(window, {
                let workspace = workspace.clone();
                let panel = panel.clone();

                move |this, window, cx| {
                    let new_position = panel.read(cx).position(window, cx);
                    if new_position == this.position {
                        return;
                    }

                    let Ok(new_dock) = workspace.update(cx, |workspace, cx| {
                        if panel.is_zoomed(window, cx) {
                            workspace.zoomed_position = Some(new_position);
                        }
                        match new_position {
                            DockPosition::Left => &workspace.left_dock,
                            DockPosition::Bottom => &workspace.bottom_dock,
                            DockPosition::Right => &workspace.right_dock,
                        }
                        .clone()
                    }) else {
                        return;
                    };

                    let was_visible = this.is_open()
                        && this.visible_panel().map_or(false, |active_panel| {
                            active_panel.panel_id() == Entity::entity_id(&panel)
                        });

                    this.remove_panel(&panel, window, cx);

                    new_dock.update(cx, |new_dock, cx| {
                        new_dock.remove_panel(&panel, window, cx);
                        let index =
                            new_dock.add_panel(panel.clone(), workspace.clone(), window, cx);
                        if was_visible {
                            new_dock.set_open(true, window, cx);
                            new_dock.activate_panel(index, window, cx);
                        }
                    });
                }
            }),
            cx.subscribe_in(
                &panel,
                window,
                move |this, panel, event, window, cx| match event {
                    PanelEvent::ZoomIn => {
                        this.set_panel_zoomed(&panel.to_any(), true, window, cx);
                        // todo! remove disambiguation here if we can
                        if !PanelHandle::panel_focus_handle(panel, cx).contains_focused(window, cx)
                        {
                            window.focus_view(&panel, cx);
                        }
                        workspace
                            .update(cx, |workspace, cx| {
                                workspace.zoomed = Some(panel.downgrade().into());
                                workspace.zoomed_position =
                                    Some(panel.read(cx).position(window, cx));
                                cx.emit(Event::ZoomChanged);
                            })
                            .ok();
                    }
                    PanelEvent::ZoomOut => {
                        this.set_panel_zoomed(&panel.to_any(), false, window, cx);
                        workspace
                            .update(cx, |workspace, cx| {
                                if workspace.zoomed_position == Some(this.position) {
                                    workspace.zoomed = None;
                                    workspace.zoomed_position = None;
                                    cx.emit(Event::ZoomChanged);
                                }
                                cx.notify();
                            })
                            .ok();
                    }
                    PanelEvent::Activate => {
                        if let Some(ix) = this
                            .panel_entries
                            .iter()
                            .position(|entry| entry.panel.panel_id() == Entity::entity_id(panel))
                        {
                            this.set_open(true, window, cx);
                            this.activate_panel(ix, window, cx);
                            window.focus_view(panel, cx);
                        }
                    }
                    PanelEvent::Close => {
                        if this
                            .visible_panel()
                            .map_or(false, |p| p.panel_id() == Entity::entity_id(panel))
                        {
                            this.set_open(false, window, cx);
                        }
                    }
                },
            ),
        ];

        let index = match self
            .panel_entries
            .binary_search_by_key(&panel.read(cx).activation_priority(), |entry| {
                entry.panel.activation_priority(cx)
            }) {
            Ok(ix) => ix,
            Err(ix) => ix,
        };
        if let Some(active_index) = self.active_panel_index.as_mut() {
            if *active_index >= index {
                *active_index += 1;
            }
        }
        self.panel_entries.insert(
            index,
            PanelEntry {
                panel: Arc::new(panel.clone()),
                _subscriptions: subscriptions,
            },
        );

        if !self.restore_state(window, cx) && panel.read(cx).starts_open(window, cx) {
            self.activate_panel(index, window, cx);
            self.set_open(true, window, cx);
        }

        cx.notify();
        index
    }

    pub fn restore_state(&mut self, window: &mut Window, cx: &mut ModelContext<Self>) -> bool {
        if let Some(serialized) = self.serialized_dock.clone() {
            if let Some(active_panel) = serialized.active_panel {
                if let Some(idx) = self.panel_index_for_persistent_name(active_panel.as_str(), cx) {
                    self.activate_panel(idx, window, cx);
                }
            }

            if serialized.zoom {
                if let Some(panel) = self.active_panel() {
                    panel.set_zoomed(true, window, cx)
                }
            }
            self.set_open(serialized.visible, window, cx);
            return true;
        }
        false
    }

    pub fn remove_panel<T: Panel>(
        &mut self,
        panel: &Model<T>,
        window: &mut Window,
        cx: &mut ModelContext<Self>,
    ) {
        if let Some(panel_ix) = self
            .panel_entries
            .iter()
            .position(|entry| entry.panel.panel_id() == Entity::entity_id(panel))
        {
            if let Some(active_panel_index) = self.active_panel_index.as_mut() {
                match panel_ix.cmp(active_panel_index) {
                    std::cmp::Ordering::Less => {
                        *active_panel_index -= 1;
                    }
                    std::cmp::Ordering::Equal => {
                        self.active_panel_index = None;
                        self.set_open(false, window, cx);
                    }
                    std::cmp::Ordering::Greater => {}
                }
            }
            self.panel_entries.remove(panel_ix);
            cx.notify();
        }
    }

    pub fn panels_len(&self) -> usize {
        self.panel_entries.len()
    }

    pub fn activate_panel(
        &mut self,
        panel_ix: usize,
        window: &mut Window,
        cx: &mut ModelContext<Self>,
    ) {
        if Some(panel_ix) != self.active_panel_index {
            if let Some(active_panel) = self.active_panel_entry() {
                active_panel.panel.set_active(false, window, cx);
            }

            self.active_panel_index = Some(panel_ix);
            if let Some(active_panel) = self.active_panel_entry() {
                active_panel.panel.set_active(true, window, cx);
            }

            cx.notify();
        }
    }

    pub fn visible_panel(&self) -> Option<&Arc<dyn PanelHandle>> {
        let entry = self.visible_entry()?;
        Some(&entry.panel)
    }

    pub fn active_panel(&self) -> Option<&Arc<dyn PanelHandle>> {
        let panel_entry = self.active_panel_entry()?;
        Some(&panel_entry.panel)
    }

    fn visible_entry(&self) -> Option<&PanelEntry> {
        if self.is_open {
            self.active_panel_entry()
        } else {
            None
        }
    }

    pub fn zoomed_panel(&self, window: &Window, cx: &AppContext) -> Option<Arc<dyn PanelHandle>> {
        let entry = self.visible_entry()?;
        if entry.panel.is_zoomed(window, cx) {
            Some(entry.panel.clone())
        } else {
            None
        }
    }

    pub fn panel_size(
        &self,
        panel: &dyn PanelHandle,
        window: &Window,
        cx: &AppContext,
    ) -> Option<Pixels> {
        self.panel_entries
            .iter()
            .find(|entry| entry.panel.panel_id() == panel.panel_id())
            .map(|entry| entry.panel.size(window, cx))
    }

    pub fn active_panel_size(&self, window: &Window, cx: &AppContext) -> Option<Pixels> {
        if self.is_open {
            self.active_panel_entry()
                .map(|entry| entry.panel.size(window, cx))
        } else {
            None
        }
    }

    pub fn resize_active_panel(
        &mut self,
        size: Option<Pixels>,
        window: &mut Window,
        cx: &mut ModelContext<Self>,
    ) {
        if let Some(entry) = self.active_panel_entry() {
            let size = size.map(|size| size.max(RESIZE_HANDLE_SIZE).round());

            entry.panel.set_size(size, window, cx);
            cx.notify();
        }
    }

    pub fn toggle_action(&self) -> Box<dyn Action> {
        match self.position {
            DockPosition::Left => crate::ToggleLeftDock.boxed_clone(),
            DockPosition::Bottom => crate::ToggleBottomDock.boxed_clone(),
            DockPosition::Right => crate::ToggleRightDock.boxed_clone(),
        }
    }

    fn dispatch_context() -> KeyContext {
        let mut dispatch_context = KeyContext::new_with_defaults();
        dispatch_context.add("Dock");

        dispatch_context
    }

    pub fn clamp_panel_size(&mut self, max_size: Pixels, window: &mut Window, cx: &mut AppContext) {
        let max_size = px((max_size.0 - RESIZE_HANDLE_SIZE.0).abs());
        for panel in self.panel_entries.iter().map(|entry| &entry.panel) {
            if panel.size(window, cx) > max_size {
                panel.set_size(Some(max_size.max(RESIZE_HANDLE_SIZE)), window, cx);
            }
        }
    }
}

impl Render for Dock {
    fn render(&mut self, window: &mut Window, cx: &mut ModelContext<Self>) -> impl IntoElement {
        let dispatch_context = Self::dispatch_context();
        if let Some(entry) = self.visible_entry() {
            let size = entry.panel.size(window, cx);

            let position = self.position;
            let create_resize_handle = || {
                let handle = div()
                    .id("resize-handle")
                    .on_drag(DraggedDock(position), |dock, _, window, cx| {
                        cx.stop_propagation();
                        window.new_view(cx, |_, _| dock.clone())
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|v, e: &MouseUpEvent, window, cx| {
                            if e.click_count == 2 {
                                v.resize_active_panel(None, window, cx);
                                cx.stop_propagation();
                            }
                        }),
                    )
                    .occlude();
                match self.position() {
                    DockPosition::Left => deferred(
                        handle
                            .absolute()
                            .right(-RESIZE_HANDLE_SIZE / 2.)
                            .top(px(0.))
                            .h_full()
                            .w(RESIZE_HANDLE_SIZE)
                            .cursor_col_resize(),
                    ),
                    DockPosition::Bottom => deferred(
                        handle
                            .absolute()
                            .top(-RESIZE_HANDLE_SIZE / 2.)
                            .left(px(0.))
                            .w_full()
                            .h(RESIZE_HANDLE_SIZE)
                            .cursor_row_resize(),
                    ),
                    DockPosition::Right => deferred(
                        handle
                            .absolute()
                            .top(px(0.))
                            .left(-RESIZE_HANDLE_SIZE / 2.)
                            .h_full()
                            .w(RESIZE_HANDLE_SIZE)
                            .cursor_col_resize(),
                    ),
                }
            };

            div()
                .key_context(dispatch_context)
                .track_focus(&self.focus_handle(cx))
                .flex()
                .bg(cx.theme().colors().panel_background)
                .border_color(cx.theme().colors().border)
                .overflow_hidden()
                .map(|this| match self.position().axis() {
                    Axis::Horizontal => this.w(size).h_full().flex_row(),
                    Axis::Vertical => this.h(size).w_full().flex_col(),
                })
                .map(|this| match self.position() {
                    DockPosition::Left => this.border_r_1(),
                    DockPosition::Right => this.border_l_1(),
                    DockPosition::Bottom => this.border_t_1(),
                })
                .child(
                    div()
                        .map(|this| match self.position().axis() {
                            Axis::Horizontal => this.min_w(size).h_full(),
                            Axis::Vertical => this.min_h(size).w_full(),
                        })
                        .child(
                            entry
                                .panel
                                .to_any()
                                .cached(StyleRefinement::default().v_flex().size_full()),
                        ),
                )
                .when(self.resizeable, |this| this.child(create_resize_handle()))
        } else {
            div()
                .key_context(dispatch_context)
                .track_focus(&self.focus_handle(cx))
        }
    }
}

impl PanelButtons {
    pub fn new(dock: Model<Dock>, window: &mut Window, cx: &mut ModelContext<Self>) -> Self {
        cx.observe_in(&dock, window, |_, _, window, cx| cx.notify())
            .detach();
        Self { dock }
    }
}

impl Render for PanelButtons {
    fn render(&mut self, window: &mut Window, cx: &mut ModelContext<Self>) -> impl IntoElement {
        let dock = self.dock.read(cx);
        let active_index = dock.active_panel_index;
        let is_open = dock.is_open;
        let dock_position = dock.position;

        let (menu_anchor, menu_attach) = match dock.position {
            DockPosition::Left => (Corner::BottomLeft, Corner::TopLeft),
            DockPosition::Bottom | DockPosition::Right => (Corner::BottomRight, Corner::TopRight),
        };

        let buttons = dock
            .panel_entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                let icon = entry.panel.icon(window, cx)?;
                let icon_tooltip = entry.panel.icon_tooltip(window, cx)?;
                let name = entry.panel.persistent_name();
                let panel = entry.panel.clone();

                let is_active_button = Some(i) == active_index && is_open;
                let (action, tooltip) = if is_active_button {
                    let action = dock.toggle_action();

                    let tooltip: SharedString =
                        format!("Close {} dock", dock.position.label()).into();

                    (action, tooltip)
                } else {
                    let action = entry.panel.toggle_action(window, cx);

                    (action, icon_tooltip.into())
                };

                Some(
                    right_click_menu(name)
                        .menu(move |window, cx| {
                            const POSITIONS: [DockPosition; 3] = [
                                DockPosition::Left,
                                DockPosition::Right,
                                DockPosition::Bottom,
                            ];

                            ContextMenu::build(window, cx, |mut menu, window, cx| {
                                for position in POSITIONS {
                                    if position != dock_position
                                        && panel.position_is_valid(position, window, cx)
                                    {
                                        let panel = panel.clone();
                                        menu = menu.entry(
                                            format!("Dock {}", position.label()),
                                            None,
                                            move |window, cx| {
                                                panel.set_position(position, window, cx);
                                            },
                                        )
                                    }
                                }
                                menu
                            })
                        })
                        .anchor(menu_anchor)
                        .attach(menu_attach)
                        .trigger(
                            IconButton::new(name, icon)
                                .icon_size(IconSize::Small)
                                .toggle_state(is_active_button)
                                .on_click({
                                    let action = action.boxed_clone();
                                    move |_, window, cx| {
                                        window.dispatch_action(action.boxed_clone(), cx)
                                    }
                                })
                                .tooltip(move |window, cx| {
                                    Tooltip::for_action(tooltip.clone(), &*action, window, cx)
                                }),
                        ),
                )
            });

        h_flex().gap_0p5().children(buttons)
    }
}

impl StatusItemView for PanelButtons {
    fn set_active_pane_item(
        &mut self,
        _active_pane_item: Option<&dyn crate::ItemHandle>,
        _window: &mut Window,
        _cx: &mut ModelContext<Self>,
    ) {
        // Nothing to do, panel buttons don't depend on the active center item
    }
}

#[cfg(any(test, feature = "test-support"))]
pub mod test {
    use super::*;
    use gpui::{actions, div, AppContext, ModelContext, Window};

    pub struct TestPanel {
        pub position: DockPosition,
        pub zoomed: bool,
        pub active: bool,
        pub focus_handle: FocusHandle,
        pub size: Pixels,
    }
    actions!(test, [ToggleTestPanel]);

    impl EventEmitter<PanelEvent> for TestPanel {}

    impl TestPanel {
        pub fn new(position: DockPosition, window: &mut Window, cx: &mut AppContext) -> Self {
            Self {
                position,
                zoomed: false,
                active: false,
                focus_handle: cx.focus_handle(),
                size: px(300.),
            }
        }
    }

    impl Render for TestPanel {
        fn render(&mut self, window: &mut Window, cx: &mut ModelContext<Self>) -> impl IntoElement {
            div().id("test").track_focus(&self.focus_handle(cx))
        }
    }

    impl Panel for TestPanel {
        fn persistent_name() -> &'static str {
            "TestPanel"
        }

        fn position(&self, _window: &Window, _: &AppContext) -> super::DockPosition {
            self.position
        }

        fn position_is_valid(&self, _: super::DockPosition) -> bool {
            true
        }

        fn set_position(
            &mut self,
            position: DockPosition,
            window: &mut Window,
            cx: &mut ModelContext<Self>,
        ) {
            self.position = position;
            cx.update_global::<SettingsStore, _>(|_, _| {});
        }

        fn size(&self, _window: &Window, _: &AppContext) -> Pixels {
            self.size
        }

        fn set_size(
            &mut self,
            size: Option<Pixels>,
            _window: &mut Window,
            _: &mut ModelContext<Self>,
        ) {
            self.size = size.unwrap_or(px(300.));
        }

        fn icon(&self, _window: &Window, _: &AppContext) -> Option<ui::IconName> {
            None
        }

        fn icon_tooltip(&self, _window: &Window, _cx: &AppContext) -> Option<&'static str> {
            None
        }

        fn toggle_action(&self) -> Box<dyn Action> {
            ToggleTestPanel.boxed_clone()
        }

        fn is_zoomed(&self, _window: &Window, _: &AppContext) -> bool {
            self.zoomed
        }

        fn set_zoomed(&mut self, zoomed: bool, _window: &mut Window, _cx: &mut ModelContext<Self>) {
            self.zoomed = zoomed;
        }

        fn set_active(&mut self, active: bool, _window: &mut Window, _cx: &mut ModelContext<Self>) {
            self.active = active;
        }

        fn activation_priority(&self) -> u32 {
            100
        }
    }

    impl FocusableView for TestPanel {
        fn focus_handle(&self, _cx: &AppContext) -> FocusHandle {
            self.focus_handle.clone()
        }
    }
}
