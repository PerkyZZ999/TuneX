pragma Singleton
import QtQuick

// NoticeBus: the one toast channel. Any actionable component announces
// through `send()` instead of relaying a `notice` signal up four hops;
// App.qml subscribes once and shows the shell Toast. Errors and infos share
// the channel; the Toast's own stickiness policy decides display.
QtObject {
    signal notice(string text, bool isError)

    function send(text, isError) {
        notice(text, isError);
    }
}
