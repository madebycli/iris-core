# Lists and notes: the next skills to move into the core. These sentences come from the Kotlin
# end-to-end test (app/src/test/java/app/iris/ui/EndToEndTest.kt) and describe what must hold.
# They do not pass yet; move a case to core/specs/ when it does.

## add to a named list
> Füge Milch zur Einkaufsliste hinzu
= item "Milch"

## the list is named by what is on it
> Setze Zelda auf die Spieleliste
= item "Zelda"

## two things at once
> Setze Brot und Eier auf die Einkaufsliste
= item "Brot"
= item "Eier"

## a note
> Notiere Milch kaufen
= note "Milch kaufen"

## guided note
> Neue Notiz
= kind question
> Milch kaufen
= note "Milch kaufen"
