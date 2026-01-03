#include <ruby.h>
#include <stdlib.h>
#include <stdbool.h>

#include "configuration.h"

static int extract_interval_ms(VALUE options_hash);
static enum pf2_time_mode extract_time_mode(VALUE options_hash);
static bool extract__test_no_install_timer(VALUE options_hash);

struct pf2_configuration *
pf2_configuration_new_from_options_hash(VALUE options_hash)
{
    struct pf2_configuration *config = malloc(sizeof(struct pf2_configuration));
    if (!config) {
        rb_raise(rb_eRuntimeError, "Failed to allocate configuration");
    }

    config->interval_ms = extract_interval_ms(options_hash);
    config->time_mode = extract_time_mode(options_hash);
    config->_test_no_install_timer = extract__test_no_install_timer(options_hash);

    return config;
}

static int
extract_interval_ms(VALUE options_hash)
{
    if (options_hash == Qnil) {
        return PF2_DEFAULT_INTERVAL_MS;
    }

    VALUE interval_ms = rb_hash_aref(options_hash, ID2SYM(rb_intern("interval_ms")));
    if (interval_ms == Qundef || interval_ms == Qnil) {
        return PF2_DEFAULT_INTERVAL_MS;
    }

    return NUM2INT(interval_ms);
}

static enum pf2_time_mode
extract_time_mode(VALUE options_hash)
{
    if (options_hash == Qnil) {
        return PF2_DEFAULT_TIME_MODE;
    }

    VALUE time_mode = rb_hash_aref(options_hash, ID2SYM(rb_intern("time_mode")));
    if (time_mode == Qundef || time_mode == Qnil) {
        return PF2_DEFAULT_TIME_MODE;
    }

    if (time_mode == ID2SYM(rb_intern("cpu"))) {
        return PF2_TIME_MODE_CPU_TIME;
    } else if (time_mode == ID2SYM(rb_intern("wall"))) {
        return PF2_TIME_MODE_WALL_TIME;
    } else {
        VALUE time_mode_str = rb_obj_as_string(time_mode);
        rb_raise(rb_eArgError, "Invalid time mode: %s", StringValueCStr(time_mode_str));
    }
}

static bool
extract__test_no_install_timer(VALUE options_hash)
{
    if (options_hash == Qnil) {
        return PF2_DEFAULT__TEST_NO_INSTALL_TIMER;
    }

    VALUE _test_no_install_timer = rb_hash_aref(options_hash, ID2SYM(rb_intern("_test_no_install_timer")));
    return RTEST(_test_no_install_timer);
}

void
pf2_configuration_free(struct pf2_configuration *config)
{
    free(config);
}

VALUE
pf2_configuration_to_ruby_hash(struct pf2_configuration *config)
{
    VALUE hash = rb_hash_new();

    // interval_ms
    rb_hash_aset(hash, ID2SYM(rb_intern("interval_ms")), INT2NUM(config->interval_ms));

    // time_mode
    VALUE time_mode_sym;
    switch (config->time_mode) {
    case PF2_TIME_MODE_CPU_TIME:
        time_mode_sym = ID2SYM(rb_intern("cpu"));
        break;
    case PF2_TIME_MODE_WALL_TIME:
        time_mode_sym = ID2SYM(rb_intern("wall"));
        break;
    default:
        rb_raise(rb_eRuntimeError, "Invalid time mode");
        break;
    }
    rb_hash_aset(hash, ID2SYM(rb_intern("time_mode")), time_mode_sym);

    return hash;
}
