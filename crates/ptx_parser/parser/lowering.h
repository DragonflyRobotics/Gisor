#ifndef LOWERING_H
#define LOWERING_H

#include "parser.h"
#include "types.h"

ParsedKernel lower(ParseOutput parsed, Arena* arena);

#endif