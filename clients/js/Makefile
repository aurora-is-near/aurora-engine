TSC = npx tsc

# The default target:

all: index.js engine.js schema.js

.PHONY: all

# Rules for development:

%.js: %.ts tsconfig.json package.json package-lock.json
	$(TSC)

%.d.ts: %.js

clean:
	@rm -Rf *.js *.d.ts *~

distclean: clean

mostlyclean: clean

maintainer-clean: clean

.PHONY: clean distclean mostlyclean maintainer-clean

.SECONDARY:
.SUFFIXES:
