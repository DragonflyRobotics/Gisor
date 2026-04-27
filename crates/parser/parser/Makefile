CFLAGS = -Wall -Wextra -O2 -g
SRCS = arena.c lexer.c parser.c lowering.c output.c main.c
OBJS = $(SRCS:.c=.o)

ptx_test: $(OBJS)
	$(CC) $(CFLAGS) $(OBJS) -o $@

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f $(OBJS) ptx_test

.PHONY: clean