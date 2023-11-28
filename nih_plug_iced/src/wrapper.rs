//! An [`Application`] wrapper around an [`IcedEditor`] to bridge between `iced_baseview` and
//! `nih_plug_iced`.

use crossbeam::channel;
use iced_baseview::Renderer;
use nih_plug::prelude::GuiContext;
use std::sync::Arc;

use crate::futures::futures::FutureExt;
use crate::{
    futures, Application, Command, Element, IcedEditor, ParameterUpdate, Subscription,
    WindowScalePolicy, WindowSubs,
};

/// Wraps an `iced_baseview` [`Application`] around [`IcedEditor`]. Needed to allow editors to
/// always receive a copy of the GUI context.
pub(crate) struct IcedEditorWrapperApplication<E: IcedEditor> {
    editor: E,

    /// We will receive notifications about parameters being changed on here. Whenever a parameter
    /// update gets sent, we will trigger a [`Message::parameterUpdate`] which causes the UI to be
    /// redrawn.
    parameter_updates_receiver: Arc<channel::Receiver<ParameterUpdate>>,
}

/// This wraps around `E::Message` to add a parameter update message which can be handled directly
/// by this wrapper. That parameter update message simply forces a redraw of the GUI whenever there
/// is a parameter update.
pub enum Message<E: IcedEditor> {
    EditorMessage(E::Message),
    ParameterUpdate,
}

impl<E: IcedEditor> std::fmt::Debug for Message<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EditorMessage(arg0) => f.debug_tuple("EditorMessage").field(arg0).finish(),
            Self::ParameterUpdate => write!(f, "ParameterUpdate"),
        }
    }
}

impl<E: IcedEditor> Clone for Message<E> {
    fn clone(&self) -> Self {
        match self {
            Self::EditorMessage(arg0) => Self::EditorMessage(arg0.clone()),
            Self::ParameterUpdate => Self::ParameterUpdate,
        }
    }
}

impl<E: IcedEditor> Application for IcedEditorWrapperApplication<E> {
    type Theme = String;
    type Executor = E::Executor;
    type Message = Message<E>;
    type Flags = (
        Arc<dyn GuiContext>,
        Arc<channel::Receiver<ParameterUpdate>>,
        E::InitializationFlags,
    );

    fn title(&self) -> String {
        return String::from("TEST");
    }

    fn new(
        (context, parameter_updates_receiver, flags): Self::Flags,
    ) -> (Self, Command<Self::Message>) {
        let (editor, command) = E::new(flags, context);

        (
            Self {
                editor,
                parameter_updates_receiver,
            },
            command.map(Message::EditorMessage),
        )
    }

    #[inline]
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::EditorMessage(message) => {
                self.editor.update(message).map(Message::EditorMessage)
            }
            // This message only exists to force a redraw
            Message::ParameterUpdate => Command::none(),
        }
    }

    #[inline]
    fn subscription(
        &self,
        window_subs: &mut WindowSubs<Self::Message>,
    ) -> Subscription<Self::Message> {
        // Since we're wrapping around `E::Message`, we need to do this transformation ourselves
        // let mut editor_window_subs = WindowSubs {
        //     on_frame: match &window_subs.on_frame {
        //         Some(function) => Some(function.clone()),
        //         _ => None,
        //     },
        //     on_window_will_close: match &window_subs.on_window_will_close {
        //         Some(function) => Some(function.clone()),
        //         _ => None,
        //     },
        // };
        let mut editor_window_subs = WindowSubs {
            on_frame: window_subs.on_frame,
            on_window_will_close: window_subs.on_window_will_close,
        };

        let subscription = Subscription::batch([
            // For some reason there's no adapter to just convert `futures::channel::mpsc::Receiver`
            // into a stream that doesn't require consuming that receiver (which wouldn't work in
            // this case since the subscriptions function gets called repeatedly). So we'll just use
            // a crossbeam queue and this unfold instead.
            futures::subscription::unfold(
                "parameter updates",
                self.parameter_updates_receiver.clone(),
                |parameter_updates_receiver| match parameter_updates_receiver.try_recv() {
                    Ok(_) => iced_baseview::futures::futures::future::ready((
                        Some(Message::ParameterUpdate),
                        parameter_updates_receiver,
                    ))
                    .boxed(),
                    Err(_) => iced_baseview::futures::futures::future::pending().boxed(),
                },
            ),
            self.editor
                .subscription(&mut editor_window_subs)
                .map(Message::EditorMessage),
        ]);

        if let Some(message) = editor_window_subs.on_frame {
            window_subs.on_frame = Some(on_frame);
        }
        if let Some(message) = editor_window_subs.on_window_will_close {
            window_subs.on_window_will_close = Some(on_window_will_close);
        }

        subscription
    }

    #[inline]
    fn view(&mut self) -> Element<'_, Self::Message, Renderer> {
        self.editor.view().map(Message::EditorMessage)
    }

    #[inline]
    fn scale_policy(&self) -> WindowScalePolicy {
        self.editor.scale_policy()
    }

    #[inline]
    fn renderer_settings() -> iced_baseview::backend::settings::Settings {
        E::renderer_settings()
    }
}

impl<E: IcedEditor> IcedEditorWrapperApplication<E> {
    fn on_frame() -> <IcedEditorWrapperApplication<E> as iced_baseview::Application>::Message {
        Message::EditorMessage(message)
    }
    fn on_window_will_close() -> <IcedEditorWrapperApplication<E> as iced_baseview::Application>::Message {
        Message::EditorMessage(message)
    }
}
