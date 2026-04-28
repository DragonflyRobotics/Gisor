#ifndef PTX_PARSER_H
#define PTX_PARSER_H

#include "types.h"

ParsedKernel ptx_parse(const char* source);
ParsedSignature parse_c_signature(const char* source);

void parse_c_signature_destroy(ParsedSignature* sig);
void ptx_parse_destroy(ParsedKernel* kernel);

#endif