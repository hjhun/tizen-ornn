#include <Elementary.h>
#include <stdio.h>

struct counter {
    Evas_Object *number;
    Evas_Object *button;
    unsigned int value;
};

static void show_count(struct counter *counter)
{
    char label[96];

    snprintf(label, sizeof(label),
             "<align=center><font_size=54>%u</font_size></align>",
             counter->value);
    elm_object_text_set(counter->number, label);
    printf("counter=%u\n", counter->value);
    fflush(stdout);
}

static void increment(void *data, Evas_Object *object, void *event)
{
    struct counter *counter = data;
    (void)object;
    (void)event;

    ++counter->value;
    show_count(counter);
}

static Eina_Bool report_button(void *data)
{
    struct counter *counter = data;
    Evas_Coord x, y, width, height;
    evas_object_geometry_get(counter->button, &x, &y, &width, &height);
    printf("button-center=%d,%d\n", x + width / 2, y + height / 2);
    fflush(stdout);
    return ECORE_CALLBACK_CANCEL;
}

int main(int argc, char **argv)
{
    struct counter counter = {0};
    Evas_Object *window, *background, *box, *title;

    if (elm_init(argc, argv) <= 0)
        return 1;
    elm_policy_set(ELM_POLICY_QUIT, ELM_POLICY_QUIT_LAST_WINDOW_CLOSED);
    window = elm_win_add(NULL, "tizen-ui", ELM_WIN_BASIC);
    if (!window) {
        fprintf(stderr, "Unable to create an Elementary window\n");
        elm_shutdown();
        return 1;
    }
    elm_win_title_set(window, "Tizen UI counter");
    elm_win_autodel_set(window, EINA_TRUE);
    elm_win_fullscreen_set(window, EINA_TRUE);

    background = elm_bg_add(window);
    elm_bg_color_set(background, 228, 237, 247);
    evas_object_size_hint_weight_set(background, EVAS_HINT_EXPAND, EVAS_HINT_EXPAND);
    elm_win_resize_object_add(window, background);
    evas_object_show(background);

    box = elm_box_add(window);
    elm_box_padding_set(box, 0, 16);
    evas_object_size_hint_weight_set(box, EVAS_HINT_EXPAND, EVAS_HINT_EXPAND);
    elm_win_resize_object_add(window, box);
    evas_object_show(box);

    title = elm_label_add(box);
    elm_object_text_set(title, "<align=center><font_size=30>Tizen UI counter</font_size></align>");
    elm_box_pack_end(box, title);
    evas_object_show(title);
    counter.number = elm_label_add(box);
    elm_box_pack_end(box, counter.number);
    evas_object_show(counter.number);
    show_count(&counter);

    counter.button = elm_button_add(box);
    elm_object_text_set(counter.button, "Add one");
    elm_box_pack_end(box, counter.button);
    evas_object_smart_callback_add(counter.button, "clicked", increment, &counter);
    evas_object_show(counter.button);

    evas_object_resize(window, 640, 480);
    evas_object_show(window);
    ecore_timer_add(1.0, report_button, &counter);
    elm_run();
    elm_shutdown();
    return 0;
}
