//! The dialog that asks for the new name of a session: one line of text over
//! the window. Enter gives the name, Escape cancels, and an empty line
//! clears the name.

use std::rc::Rc;

use gpui::{
    App, ClipboardItem, Context, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, Styled, Window, div,
};
use nomux_desktop_model::LineEditor;
use nomux_desktop_theme::{ColorToken, DesktopTheme, LengthToken, TextStyle};

/// What the dialog tells its owner: the new name, or `None` to clear it.
pub(crate) type SubmitHandler = Rc<dyn Fn(Option<String>, &mut Window, &mut App)>;
/// The dialog was cancelled.
pub(crate) type CancelHandler = Rc<dyn Fn(&mut Window, &mut App)>;

pub(crate) struct RenameDialog {
    theme: DesktopTheme,
    editor: LineEditor,
    focus: FocusHandle,
    on_submit: SubmitHandler,
    on_cancel: CancelHandler,
}

impl RenameDialog {
    pub(crate) fn new(
        theme: DesktopTheme,
        name: &str,
        handlers: (SubmitHandler, CancelHandler),
        cx: &mut Context<Self>,
    ) -> Self {
        let (on_submit, on_cancel) = handlers;
        Self {
            theme,
            editor: LineEditor::new(name),
            focus: cx.focus_handle(),
            on_submit,
            on_cancel,
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        match keystroke.key.as_str() {
            "enter" => {
                let name = self.editor.text().trim();
                let name = (!name.is_empty()).then(|| name.to_owned());
                (self.on_submit)(name, window, cx);
            }
            "escape" => (self.on_cancel)(window, cx),
            "backspace" => self.editor.backspace(),
            "delete" => self.editor.delete(),
            "left" => self.editor.left(),
            "right" => self.editor.right(),
            "home" => self.editor.home(),
            "end" => self.editor.end(),
            "v" if keystroke.modifiers.secondary(cx) => self.paste(cx),
            _ if keystroke.modifiers.secondary(cx) => return,
            _ => match &keystroke.key_char {
                Some(text) => self.editor.insert(text),
                None => return,
            },
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx
            .read_from_clipboard()
            .and_then(|item: ClipboardItem| item.text())
        {
            self.editor.insert(&text);
        }
    }
}

impl Focusable for RenameDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for RenameDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;
        let metrics = theme.metrics();
        let title = theme.font(TextStyle::Title);
        let row = theme.font(TextStyle::Row);
        let meta = theme.font(TextStyle::Meta);
        let (before, after) = self.editor.split();
        let text = |style: gpui::Div| {
            style
                .font_family(row.family.clone())
                .text_size(theme.font_size(TextStyle::Row))
                .text_color(theme.color(ColorToken::TextPrimary))
        };
        let cursor = div()
            .w(theme.length(LengthToken::FocusRingWidth))
            .h(theme.length(LengthToken::SizeIcon))
            .bg(theme.color(ColorToken::Accent));
        div()
            .id("rename-dialog")
            .absolute()
            .inset_0()
            .flex()
            .items_start()
            .justify_center()
            .pt(theme.length(LengthToken::Space4) * 4.0)
            .occlude()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.length(LengthToken::Space2))
                    .w(theme.length(LengthToken::Space4) * 18.0)
                    .p(metrics.padding)
                    .rounded(theme.length(LengthToken::RadiusRaised))
                    .border(theme.length(LengthToken::LineWidth))
                    .border_color(theme.color(ColorToken::LineDivider))
                    .bg(theme.color(ColorToken::SurfaceRaised))
                    .child(
                        div()
                            .font_family(title.family)
                            .font_weight(title.weight)
                            .text_size(theme.font_size(TextStyle::Title))
                            .text_color(theme.color(ColorToken::TextPrimary))
                            .child("Rename session"),
                    )
                    .child(
                        text(div())
                            .flex()
                            .items_center()
                            .h(metrics.control_height)
                            .px(metrics.padding)
                            .rounded(theme.length(LengthToken::RadiusButton))
                            .bg(theme.color(ColorToken::SurfaceSunken))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(before.to_owned())
                            .child(cursor)
                            .child(after.to_owned()),
                    )
                    .child(
                        div()
                            .font_family(meta.family)
                            .text_size(theme.font_size(TextStyle::Meta))
                            .text_color(theme.color(ColorToken::TextMuted))
                            .child("Enter renames. Escape cancels. An empty name clears the name."),
                    ),
            )
    }
}
