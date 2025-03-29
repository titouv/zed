use crate::commit_tooltip::CommitTooltip;
use crate::commit_view::CommitView;
use editor::{BlameRenderer, Editor};
use git::{
    blame::{BlameEntry, ParsedCommitMessage},
    repository::CommitSummary,
};
use gpui::{
    AnyElement, App, AppContext as _, Div, Entity, Stateful, StatefulInteractiveElement as _,
    Styled as _, WeakEntity,
};
use project::git_store::Repository;
use ui::Element as _;
use workspace::Workspace;

pub struct GitBlameRenderer;

impl BlameRenderer for GitBlameRenderer {
    fn render_blame_entry(
        &self,
        div: Stateful<Div>,
        blame_entry: BlameEntry,
        details: Option<ParsedCommitMessage>,
        repository: Entity<Repository>,
        workspace: WeakEntity<Workspace>,
        _cx: &App,
    ) -> AnyElement {
        div.cursor_pointer()
            .on_click({
                let blame_entry = blame_entry.clone();
                let repository = repository.clone();
                let workspace = workspace.clone();
                move |_, window, cx| {
                    CommitView::open(
                        CommitSummary {
                            sha: blame_entry.sha.to_string().into(),
                            subject: blame_entry.summary.clone().unwrap_or_default().into(),
                            commit_timestamp: blame_entry.committer_time.unwrap_or_default(),
                            has_parent: true,
                        },
                        repository.downgrade(),
                        workspace.clone(),
                        window,
                        cx,
                    )
                }
            })
            .hoverable_tooltip(move |window, cx| {
                cx.new(|cx| {
                    CommitTooltip::blame_entry(
                        &blame_entry,
                        details.clone(),
                        repository.clone(),
                        workspace.clone(),
                        window,
                        cx,
                    )
                })
                .into()
            })
            .into_any()
    }

    fn render_inline_blame_entry(
        &self,
        div: Stateful<Div>,
        blame_entry: BlameEntry,
        details: Option<ParsedCommitMessage>,
        repository: Entity<Repository>,
        workspace: WeakEntity<Workspace>,
        editor: Entity<Editor>,
        _: &App,
    ) -> gpui::AnyElement {
        div.hoverable_tooltip(move |window, cx| {
            let tooltip = cx.new(|cx| {
                CommitTooltip::blame_entry(
                    &blame_entry,
                    details.clone(),
                    repository.clone(),
                    workspace.clone(),
                    window,
                    cx,
                )
            });
            editor.update(cx, |editor, _| {
                editor.git_blame_inline_tooltip = Some(tooltip.downgrade().into())
            });
            tooltip.into()
        })
        .into_any()
    }
}
