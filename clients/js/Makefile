TSC = npx tsc

# The default target:

all: engine.js

.PHONY: all

# Rules for development:

engine.js: engine.ts tsconfig.json
	$(TSC)

engine.d.ts: engine.js

clean:
	@rm -Rf engine.js engine.d.ts *~

distclean: clean

mostlyclean: clean

maintainer-clean: clean

.PHONY: clean distclean mostlyclean maintainer-clean

.SECONDARY:
.SUFFIXES:
