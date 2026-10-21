#include <stdio.h>
#include <ppd/ppd.h>

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    ppd_file_t *ppd = ppdOpenFile(argv[1]);
    if (!ppd) return 3;
    ppdMarkDefaults(ppd);
    cups_option_t *options = NULL;
    int count = cupsParseOptions(argv[2], 0, &options);
    ppdMarkOptions(ppd, count, options);
    ppd_choice_t *choice = ppdFindMarkedChoice(ppd, "PageSize");
    ppd_size_t *size = ppdPageSize(ppd, NULL);
    printf("PageSize=%s", choice ? choice->choice : "(none)");
    if (size) printf(" size=%s %.9g x %.9g", size->name, size->width, size->length);
    else printf(" size=(none)");
    printf("\n");
    cupsFreeOptions(count, options);
    ppdClose(ppd);
    return 0;
}
