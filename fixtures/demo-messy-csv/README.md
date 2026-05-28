# Messy CSV Demo

Synthetic farmer registry source with government-file problems:

- duplicate `Notes` headers;
- a blank trailing header;
- inconsistent row lengths;
- missing district and registration date values;
- duplicate farmer identifiers;
- high-sensitivity names that must be redacted in reports and previews.
- unknown extra note columns that default to redacted top values until a recipe
  records sensitivity explicitly.

This demo is intentionally synthetic. It is safe for local replay and package
export tests.
