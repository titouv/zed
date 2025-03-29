use std::{fs, path::Path};

use anyhow::Context as _;
use gpui::{App, AppContext as _, Context, Entity, Task, Window};
use language::{Capability, Language};
use multi_buffer::MultiBuffer;
use project::lsp_store::{lsp_ext_command::ExpandMacro, rust_analyzer_ext::RUST_ANALYZER_NAME};
use text::ToPointUtf16;

use crate::{
    element::register_action, lsp_ext::find_specific_language_server_in_selection, Editor,
    ExpandMacroRecursively, OpenDocs,
};

fn is_rust_language(language: &Language) -> bool {
    language.name() == "Rust".into()
}

pub fn apply_related_actions(
    editor: &Entity<Editor>,
    window: &mut Window,
    cx: &mut App,
) -> Task<()> {
    let task = editor.update(cx, |editor, cx| {
        find_specific_language_server_in_selection(editor, cx, is_rust_language, RUST_ANALYZER_NAME)
    });

    let editor = editor.clone();
    window.spawn(cx, async move |cx| {
        if task.await.is_some() {
            cx.update(|window, _| {
                register_action(&editor, window, expand_macro_recursively);
                register_action(&editor, window, open_docs);
            })
            .ok();
        }
    })
}

pub fn expand_macro_recursively(
    editor: &mut Editor,
    _: &ExpandMacroRecursively,
    window: &mut Window,
    cx: &mut Context<Editor>,
) {
    if editor.selections.count() == 0 {
        return;
    }
    let Some(project) = &editor.project else {
        return;
    };
    let Some(workspace) = editor.workspace() else {
        return;
    };

    let server_lookup = find_specific_language_server_in_selection(
        editor,
        cx,
        is_rust_language,
        RUST_ANALYZER_NAME,
    );

    let project = project.clone();
    cx.spawn_in(window, async move |_editor, cx| {
        let Some((trigger_anchor, rust_language, server_to_query, buffer)) = server_lookup.await
        else {
            return Ok(());
        };
        let buffer_snapshot = buffer.update(cx, |buffer, _| buffer.snapshot())?;
        let position = trigger_anchor.text_anchor.to_point_utf16(&buffer_snapshot);
        let expand_macro_task = project.update(cx, |project, cx| {
            project.request_lsp(
                buffer,
                project::LanguageServerToQuery::Other(server_to_query),
                ExpandMacro { position },
                cx,
            )
        })?;

        let macro_expansion = expand_macro_task.await.context("expand macro")?;
        if macro_expansion.is_empty() {
            log::info!("Empty macro expansion for position {position:?}");
            return Ok(());
        }

        let buffer = project
            .update(cx, |project, cx| project.create_buffer(cx))?
            .await?;
        workspace.update_in(cx, |workspace, window, cx| {
            buffer.update(cx, |buffer, cx| {
                buffer.set_text(macro_expansion.expansion, cx);
                buffer.set_language(Some(rust_language), cx);
                buffer.set_capability(Capability::ReadOnly, cx);
            });
            let multibuffer =
                cx.new(|cx| MultiBuffer::singleton(buffer, cx).with_title(macro_expansion.name));
            workspace.add_item_to_active_pane(
                Box::new(cx.new(|cx| {
                    let mut editor = Editor::for_multibuffer(multibuffer, None, window, cx);
                    editor.set_read_only(true);
                    editor
                })),
                None,
                true,
                window,
                cx,
            );
        })
    })
    .detach_and_log_err(cx);
}

pub fn open_docs(editor: &mut Editor, _: &OpenDocs, window: &mut Window, cx: &mut Context<Editor>) {
    if editor.selections.count() == 0 {
        return;
    }
    let Some(project) = &editor.project else {
        return;
    };
    let Some(workspace) = editor.workspace() else {
        return;
    };

    let server_lookup = find_specific_language_server_in_selection(
        editor,
        cx,
        is_rust_language,
        RUST_ANALYZER_NAME,
    );

    let project = project.clone();
    cx.spawn_in(window, async move |_editor, cx| {
        let Some((trigger_anchor, _, server_to_query, buffer)) = server_lookup.await else {
            return Ok(());
        };
        let buffer_snapshot = buffer.update(cx, |buffer, _| buffer.snapshot())?;
        let position = trigger_anchor.text_anchor.to_point_utf16(&buffer_snapshot);
        let docs_urls = project
            .update(cx, |project, cx| {
                project.request_lsp(
                    buffer,
                    project::LanguageServerToQuery::Other(server_to_query),
                    project::lsp_store::lsp_ext_command::OpenDocs { position },
                    cx,
                )
            })?
            .await
            .context("open docs")?;
        if docs_urls.is_empty() {
            log::debug!("Empty docs urls for position {position:?}");
            return Ok(());
        } else {
            log::debug!("{:?}", docs_urls);
        }

        workspace.update(cx, |_workspace, cx| {
            // Check if the local document exists, otherwise fallback to the online document.
            // Open with the default browser.
            if let Some(local_url) = docs_urls.local {
                if fs::metadata(Path::new(&local_url[8..])).is_ok() {
                    cx.open_url(&local_url);
                    return;
                }
            }

            if let Some(web_url) = docs_urls.web {
                cx.open_url(&web_url);
            }
        })
    })
    .detach_and_log_err(cx);
}
