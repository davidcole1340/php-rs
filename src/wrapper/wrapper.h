#include "php.h"
#include "ext/standard/info.h"
#include "zend_exceptions.h"

zend_string *php_rs_zend_string_init(const char *str, size_t len, bool persistent);
void php_rs_zend_string_release(zend_string *zs);
const char *php_rs_php_build_id();