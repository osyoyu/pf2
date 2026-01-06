#ifndef PF2_CONFIGURATION_H
#define PF2_CONFIGURATION_H

#include <ruby.h>
#include <stdbool.h>

enum pf2_time_mode {
    PF2_TIME_MODE_CPU_TIME,
    PF2_TIME_MODE_WALL_TIME,
};

struct pf2_configuration {
    int interval_ms;
    enum pf2_time_mode time_mode;
    bool _test_no_install_timer; // for testing only
};

#define PF2_DEFAULT_INTERVAL_MS 9
#define PF2_DEFAULT_TIME_MODE PF2_TIME_MODE_CPU_TIME
#define PF2_DEFAULT__TEST_NO_INSTALL_TIMER false

struct pf2_configuration *pf2_configuration_new_from_options_hash(VALUE options_hash);
void pf2_configuration_free(struct pf2_configuration *config);
VALUE pf2_configuration_to_ruby_hash(struct pf2_configuration *config);

#endif // PF2_CONFIGURATION_H
