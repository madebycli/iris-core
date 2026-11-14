# Lists in German. Sentences from the Kotlin end-to-end test, now held by the core.

## add 1
> Füge Milch zur Einkaufsliste hinzu
= item "Milch" list=Einkauf
= count item 1

## add 2
> Setze Milch auf die Einkaufsliste
= item "Milch" list=Einkauf
= count item 1

## add 3
> Schreib mir bitte Milch auf die Einkaufsliste
= item "Milch" list=Einkauf
= count item 1

## add 4
> Packe Milch auf die Liste Einkauf
= item "Milch" list=Einkauf
= count item 1

## add 5
> Ich bräuchte noch Milch auf meinem Einkaufszettel
= item "Milch" list=Einkauf
= count item 1

## add 6
> Auf meiner Einkaufsliste fehlt noch Milch
= item "Milch" list=Einkauf
= count item 1

## add 7
> Vergiss nicht Milch auf die Liste zu setzen
= item "Milch" list=Einkauf
= count item 1

## add 8
> Ich muss Milch kaufen
= item "Milch" list=Einkauf
= count item 1

## add 9
> Kannst du Milch zur Einkaufsliste hinzufügen
= item "Milch" list=Einkauf
= count item 1

## add 10
> Milch auf die Einkaufsliste
= item "Milch" list=Einkauf
= count item 1

## add 11
> Setz Milch auf die Liste
= item "Milch" list=Einkauf
= count item 1

## add 12
> Füge zur Einkaufsliste Milch hinzu
= item "Milch" list=Einkauf
= count item 1

## add 13
> Notiere Milch auf der Einkaufsliste
= item "Milch" list=Einkauf
= count item 1

## add 14
> Tu Milch auf die Einkaufsliste
= item "Milch" list=Einkauf
= count item 1

## add 15
> Ähm, setze Milch auf die Einkaufsliste
= item "Milch" list=Einkauf
= count item 1

## add 16
> Pack mal bitte Milch auf meinen Einkaufszettel rauf
= item "Milch" list=Einkauf
= count item 1

## add 17
> Schreib Milch auf die Einkaufsliste drauf
= item "Milch" list=Einkauf
= count item 1

## add 18
> Auf die Einkaufsliste noch Milch bitte
= item "Milch" list=Einkauf
= count item 1

## add 19
> Ich brauch noch Milch, schreib die auf meine Einkaufsliste
= item "Milch" list=Einkauf
= count item 1

## the list is named by what is on it
> Füge Zelda zur Spieleliste hinzu
= item "Zelda" list=Spiele

## Liste Spiele
> Setze Zelda auf die Liste Spiele
= item "Zelda" list=Spiele

## two things at once
> Setze Brot und Eier auf die Einkaufsliste
= item "Brot" list=Einkauf
= item "Eier" list=Einkauf

## three, with commas
> Schreib Brot, Eier und Käse auf die Einkaufsliste
= count item 3

## guided entry
> Erstelle einen neuen Eintrag
< welche Liste
> Einkaufsliste
< Was soll
> Milch
< Noch etwas
> Nein
= item "Milch" list=Einkauf
= count item 1

## read
@item "Milch" Einkauf
@item "Brot" Einkauf
@item "Zelda" Spiele
> Was steht auf meiner Einkaufsliste
< Milch
< Brot
<! Zelda

## read, other wordings
@item "Milch" Einkauf
@item "Zelda" Spiele
> Zeig mir meine Einkaufsliste
< Milch
> Was muss ich einkaufen
< Milch
> Einkaufsliste
< Milch
> Meine Einkaufsliste bitte
< Milch
> Was steht auf der Liste Einkauf
< Milch
> Was steht auf der Spieleliste
< Zelda

## read without a name: the fullest list
@item "Milch" Einkauf
@item "Brot" Einkauf
@item "Zelda" Spiele
> Gib mir mal meine Liste
< Milch

## a conversation: add, more, read, tick, delete
> Füge Milch zur Einkaufsliste hinzu
> und noch Brot
> dazu Eier
> Was steht auf der Liste
< Milch
< Brot
< Eier
> Hake Brot ab
> Lösche Eier
> Was steht auf meiner Einkaufsliste
< Milch
<! Brot
<! Eier
= item "Brot" done
= item "Milch" open
= count item 1

## the games conversation
> Erstelle einen neuen Eintrag
> Spiele
> Zelda
> Und noch Mario auf die Liste
> ja
> Tetris und Pong
> Das war's
> Setze Katze auf die Liste
= count item 5 list=Spiele
<! war

## ending words are never stored
> Erstelle einen neuen Eintrag
> Einkauf
> Milch
> das reicht
= count item 1

## cancel in a guided entry
> Erstelle einen neuen Eintrag
> Abbrechen
= count item 0

## tick, in several wordings
@item "Milch" Einkauf
@item "Brot" Einkauf
> Hake Milch ab
= item "Milch" done
= item "Brot" open

## tick: Milch abhaken
@item "Milch" Einkauf
> Milch abhaken
= item "Milch" done

## tick: ist erledigt
@item "Milch" Einkauf
> Milch ist erledigt
= item "Milch" done

## tick: a speech-recognition slip
@item "Milch" Einkauf
> Hacke Milch ab
= item "Milch" done

## delete, in several wordings
@item "Milch" Einkauf
@item "Brot" Einkauf
> Lösche Milch von der Einkaufsliste
= count item 1
= item "Brot"

## delete: streich
@item "Milch" Einkauf
> Streich Milch von meinem Einkaufszettel
= count item 0

## delete: entferne
@item "Milch" Einkauf
> Entferne Milch aus der Einkaufsliste
= count item 0

## delete: bare
@item "Milch" Einkauf
> Lösche Milch
= count item 0

## empty a list: asks first
@item "Milch" Einkauf
@item "Brot" Einkauf
> Lösche alles auf der Einkaufsliste
= kind question
= count item 2
> Ja
= count item 0

## empty a list: no
@item "Milch" Einkauf
> Leere die Einkaufsliste
> Nein
= count item 1

## empty: alle Einträge in der Liste
@item "Milch" Einkauf
@item "Brot" Einkauf
> Bitte lösche alle Einträge in der Liste Einkauf
> Ja
= count item 0

## tick everything
@item "Milch" Einkauf
@item "Brot" Einkauf
> Hake alles auf der Liste Einkauf ab
= item "Milch" done
= item "Brot" done

## today's entries
> Füge Milch zur Einkaufsliste hinzu
> Füge Brot zur Einkaufsliste hinzu
> Lösche alle Einträge von heute
> Ja
= count item 0

## tick today's entries
> Füge Milch zur Einkaufsliste hinzu
> Füge Brot zur Einkaufsliste hinzu
> Hake alle Einträge von heute ab
= item "Milch" done
= item "Brot" done

## deleting a timer is not the lists' business
> Timer für 5 Minuten
> Lösche den Timer
= count timer 0

## deleting all reminders is not the lists' business
> Erinnere mich um 17 Uhr an Oma
> Lösche alle Erinnerungen
< Erinnerung

## a command that starts with "und" is not an entry (from the phone log)
> Füge Döner zu meiner Einkaufsliste hinzu
> Und erstelle einen Wecker in 10 Minuten
= count item 1
= kind unhandled

## a speech-recognition slip in the list word (from the phone log)
> Hüge Döner zu meiner Einkaufsless hinzu
= item "Döner" list=Einkauf

## "und" continues the list with plain words
> Füge Döner zu meiner Einkaufsliste hinzu
> Und Fortnite Skins
> Und Brawl Stars Robots
= item "Fortnite Skins" list=Einkauf
= item "Brawl Stars Robots" list=Einkauf
