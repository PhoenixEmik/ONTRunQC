/* Optional loaded-image/symbol provenance, not a numerical implementation.
 * macOS: clang -dynamiclib -O2 backend_probe.c -o backend_probe.dylib
 * Linux: cc -shared -fPIC -O2 backend_probe.c -ldl -lm -o backend_probe.so
 * R: dyn.load(...); .C("ontrunqc_backend_probe", output_path)
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <math.h>
#ifdef __APPLE__
#include <mach-o/dyld.h>
#else
#include <link.h>
static int image(struct dl_phdr_info *info, size_t size, void *stream) {
    (void)size;
    fprintf(stream, "image\t%s\n", info->dlpi_name);
    return 0;
}
#endif

void ontrunqc_backend_probe(char **path) {
    FILE *stream = fopen(*path, "w");
    if (!stream) return;
#ifdef __APPLE__
    for (unsigned i = 0; i < _dyld_image_count(); ++i)
        fprintf(stream, "image\t%s\n", _dyld_get_image_name(i));
#else
    dl_iterate_phdr(image, stream);
#endif
    const char *symbols[] = {"dgemm_", "ddot_", "dsyevr_", "dsyevd_", "dpotrf_"};
    for (unsigned i = 0; i < sizeof(symbols)/sizeof(symbols[0]); ++i) {
        void *address = dlsym(RTLD_DEFAULT, symbols[i]);
        Dl_info info;
        fprintf(stream, "symbol\t%s\t%s\n", symbols[i],
                address && dladdr(address, &info) ? info.dli_fname : "NOT_RESOLVED");
    }
    fclose(stream);
}

/* Exact representable steps for the optional knot-sensitivity experiment. */
void ontrunqc_nextafter(double *value, int *direction) {
    *value = nextafter(*value, *direction < 0 ? -HUGE_VAL : HUGE_VAL);
}
