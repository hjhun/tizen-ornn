#include <glib.h>
#include <string.h>

int main(void) {
  char* result = g_ascii_strup("tizen", -1);
  int failed = strcmp(result, "TIZEN") != 0;

  g_print("C + GLib %s, pointer bits %u\n", result,
          (unsigned)(sizeof(void*) * 8));
  g_free(result);
  return failed;
}
