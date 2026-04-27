#ifndef PTX_PARSER_H
#define PTX_PARSER_H

#include "types.h"

ParsedKernel ptx_parse(const char* source);
void ptx_parse_destroy(ParsedKernel* k);

#endif