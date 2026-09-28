//! Status rows. Library definitions own status order and labels; the board
//! reads the same `Library::statuses`, so both always show the same list.
use super::collections::NameTarget;
use super::*;
use gamesync_desktop::bulk::Change;

impl LibrarySidebar {
    pub(super) fn status_row(
        &self,
        key: String,
        index: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let entity = cx.entity();
        let disabled =
            self.busy || self.edit.is_some() || self.library.read(cx).write_issue.is_some();
        let icon = match key.as_str() {
            "completed" => IconName::CircleCheck,
            "dropped" => IconName::CircleX,
            _ => IconName::Folder,
        };
        let drop_key = key.clone();
        div()
            .id(gpui::SharedString::from(format!("status-{key}")))
            .rounded(cx.theme().radius)
            .when(!disabled, |row| {
                row.on_drop(cx.listener(
                    move |this, game: &super::super::grid::DraggedGame, _, cx| {
                        this.drop_game(game, Change::Status(drop_key.clone()), cx)
                    },
                ))
            })
            .child(self.row(Scope::Status(key.clone()), icon, cx))
            .context_menu(move |menu, _, _| {
                let (rename, up, down) = (entity.clone(), entity.clone(), entity.clone());
                let (rename_key, up_key, down_key) = (key.clone(), key.clone(), key.clone());
                menu.item(PopupMenuItem::new("Rename").disabled(disabled).on_click(
                    move |_, window, cx| {
                        rename.update(cx, |this, cx| {
                            this.begin_name(NameTarget::Status(rename_key.clone()), window, cx)
                        })
                    },
                ))
                .separator()
                .item(
                    PopupMenuItem::new("Move up")
                        .disabled(disabled || index == 0)
                        .on_click(move |_, _, cx| {
                            up.update(cx, |this, cx| this.move_status(&up_key, false, cx))
                        }),
                )
                .item(
                    PopupMenuItem::new("Move down")
                        .disabled(disabled || index + 1 >= count)
                        .on_click(move |_, _, cx| {
                            down.update(cx, |this, cx| this.move_status(&down_key, true, cx))
                        }),
                )
            })
            .into_any_element()
    }

    /// Move a status one place in the shared order. `later` moves it down in
    /// the sidebar and right on the board.
    pub fn move_status(&mut self, key: &str, later: bool, cx: &mut Context<Self>) {
        if self.busy || self.edit.is_some() {
            return;
        }
        let library = self.library.read(cx);
        if let Some(issue) = &library.write_issue {
            self.message = format!("Changes unavailable: {issue}");
            cx.notify();
            return;
        }
        let Some((root, mut base)) = library.source.clone() else {
            return;
        };
        if gamesync_desktop::board::move_status(&mut base.definitions, key, later) {
            self.write_definitions(root, base, cx);
        }
    }
}
