# More ways of saying it, from the Kotlin storm test (rounds 1-12), as the speech recogniser and people say them.

## a space inside the list name
> füge milch zu meiner einkaufs liste hinzu
= item "Milch" list=Einkauf

## "hin zu" written apart
> Füge Milch zu meiner Einkaufsliste hin zu
= item "Milch" list=Einkauf

## "ich möchte gerne …"
> Ich möchte gerne Milch auf die Einkaufsliste setzen
= item "Milch" list=Einkauf

## a polite word after a comma
> Füge Milch zur Einkaufsliste hinzu, danke
= item "Milch" list=Einkauf
= count item 1

## "nimm … auf" adds
> Nimm Milch auf die Einkaufsliste auf
= item "Milch" list=Einkauf

## umlauts written out
> Fuege Milch zur Einkaufsliste hinzu
= item "Milch" list=Einkauf

## a wrong conjugation
> Setzt Milch auf die Einkaufsliste
= item "Milch" list=Einkauf

## spoken to the assistant by name
> Hey Iris, setze Milch auf die Einkaufsliste
= item "Milch" list=Einkauf

## the list name split in two
> Setze Milch auf die Einkaufs Liste
= item "Milch" list=Einkauf

## all lists
@item "Milch" Einkauf
@item "Zelda" Spiele
> Zeig meine Listen
< Milch
< Zelda

## which lists
@item "Milch" Einkauf
@item "Zelda" Spiele
> Welche Listen habe ich
< Milch
< Zelda

## tick everything without naming the list
@item "Milch" Einkauf
@item "Brot" Einkauf
> Hake alles ab
= item "Milch" done
= item "Brot" done

## tick a list by naming it
@item "Milch" Einkauf
@item "Brot" Einkauf
> Hake die Einkaufsliste ab
= item "Milch" done
= item "Brot" done

## tick, other word order
@item "Milch" Einkauf
@item "Brot" Einkauf
> Alles abhaken
= item "Milch" done
= item "Brot" done

## empty a list by naming it
@item "Milch" Einkauf
@item "Brot" Einkauf
> Lösche die Einkaufsliste
> Ja
= count item 0

## empty a list, other word order
@item "Milch" Einkauf
@item "Brot" Einkauf
> Alles von der Einkaufsliste löschen
> Ja
= count item 0

## deleting one entry is not emptying the list
@item "Milch" Einkauf
@item "Brot" Einkauf
> Lösche Milch von der Einkaufsliste
= count item 1

## a note with the verb at the front and the target at the end
> Füge Milch kaufen zu meinen Notizen hinzu
= note "Milch kaufen"

## a note: write into my notes
> Schreibe Milch kaufen in meine Notizen
= note "Milch kaufen"

## a note: put on my notes
> Setze Milch kaufen auf meine Notizen
= note "Milch kaufen"

## a note: "write down:"
> Schreib auf: Milch kaufen
= note "Milch kaufen"

## a note: save the note
> Speichere die Notiz Milch kaufen
= note "Milch kaufen"

## a note, guided: make a new note
> Mach eine neue Notiz
= kind question
> Milch kaufen
= note "Milch kaufen"

## a note, guided: add a note
> Füge eine Notiz hinzu
= kind question
> Milch kaufen
= note "Milch kaufen"

## reading notes, other words
> Notiere Wlan Passwort
> Was steht in meinen Notizen
< Wlan
> Was habe ich mir notiert
< Wlan

## deleting a note
> Notiere Milch kaufen
> Notiere Wlan Passwort
> Lösche die Notiz Milch kaufen
= count note 1
= note "Wlan Passwort"

## a timer asks for its name when it was asked for step by step
> Erstelle einen Timer
< lange
> zehn Minuten
< heißen
> Nudeln
= timer "Nudeln" 600s

## the name of a step-by-step timer can be skipped
> Erstelle einen Timer
> zehn Minuten
> Nein
= timer "Timer" 600s

## a reminder: "create … for 16:30 …"
> Kannst du mir eine Erinnerung erstellen für 16:30 Hund rausbringen
= reminder "Hund rausbringen" 2026-11-10T16:30

## a reminder: "set one for …"
> Setze eine Erinnerung für 17 Uhr Mama anrufen
= reminder "Mama anrufen" 2026-11-10T17:00

## today at a glance
> Erinnere mich in 30 Minuten an die Wäsche
@item "Milch" Einkauf
> Was muss ich heute tun?
< Wäsche
< offen

## ticking one entry, the list named
@item "Milch" Einkauf
@item "Brot" Einkauf
> Hake Milch auf der Einkaufsliste ab
= item "Milch" done
= item "Brot" open

## ticking one entry, "list Einkauf"
@item "Milch" Einkauf
@item "Brot" Einkauf
> Hake Milch von der Liste Einkauf ab
= item "Milch" done
= item "Brot" open

## deleting a note by a part of its title
> Notiere Milch kaufen
> Lösche die Notiz Milch
= count note 0

## "egal" skips the name of a step-by-step timer
> Timer
> zehn Minuten
> Egal
= timer "Timer" 600s
