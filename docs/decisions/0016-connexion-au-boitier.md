# 0016. Connexion au boîtier sous Windows

Date : 2026-10-31 (complétée le même jour après l'essai du débranchement) · Statut : **adoptée**, au choix de l'utilisateur, d'après deux sondes avec son Fujifilm X-H2

## Contexte

V0.8 : une photo déclenchée sur le boîtier relié en USB doit arriver d'elle-même dans Afterdusk, d'abord sous Windows, sans remplacer le pilote USB (cahier des charges, section 7), avec le boîtier de l'utilisateur, un Fujifilm X-H2 (micrologiciel 5.30) en mode « prise de vue reliée USB ».

Ce que les sondes (`examples/tether_probe.rs`, `examples/ptp_probe.rs`) ont montré :

- Windows Portable Devices voit le boîtier comme un appareil MTP, mais en prise de vue reliée il n'en expose aucun fichier et n'annonce rien quand on déclenche.
- Le protocole de l'appareil photo (PTP) passe en revanche par les « commandes MTP étendues » de Windows Portable Devices, sans changer de pilote. Windows ne les accepte que d'un programme qui a ouvert l'appareil avec droit d'écriture, même pour lire.
- Le boîtier n'émet aucune annonce PTP. Interrogé toutes les demi-secondes, il signale chaque déclenchement par sa propriété 0xD212, et chaque photo apparaît dans la liste de ses objets (par exemple `DSCF0001.raf`, 83 Mo, et `DSCF0002.jpg`, 17 Mo, en RAW + JPEG).
- Les photos sont enregistrées sur la carte du boîtier. La valeur D20E de 0xD212 est le nombre de photos que la carte peut encore recevoir ; à zéro, le boîtier affiche « carte pleine ».

## Décision

1. **PTP à travers Windows Portable Devices** : pas de pilote remplacé, pas de bibliothèque tierce.
2. **Droit d'écriture à l'ouverture, opérations de lecture seulement** (accord de l'utilisateur, 2026-10-31) : une liste fermée d'opérations PTP, toutes de lecture (fiche d'identité, mémoires, liste et fiche des objets, lecture d'un objet, valeur d'une propriété), vérifiée par un test. Aucune opération qui écrive ou supprime n'est jamais envoyée.
3. **Relevé toutes les demi-secondes** de la propriété 0xD212 et de la liste des objets ; chaque nouvel objet est lu, écrit dans la bibliothèque, vérifié par somme de contrôle, puis importé par le chemin habituel dans l'album de la séance.
4. **Les photos restent sur la carte** : Afterdusk ne fait que les copier. Chaque photo est copiée dès qu'elle paraît, car le boîtier ne liste que les photos prises depuis le branchement : à chaque rebranchement, sa liste repart de zéro et ses noms de `DSCF0001` (essai du 2026-10-31). Une photo prise câble débranché, ou pas encore copiée au moment d'un débranchement, reste sur la carte mais n'est plus visible par la connexion : Afterdusk le signale au rebranchement et propose de l'importer depuis la carte. Les photos déjà dans la bibliothèque y sont reconnues et ne sont pas importées deux fois.
5. **Réglage conseillé du boîtier** : profil de connexion « 3: USB TETHER SHOOTING AUTO ». D'après le manuel du X-H2, câble débranché, le boîtier enregistre alors sur la carte ; en « 4: USB TETHER SHOOTING FIXED », il reste en prise de vue reliée et n'enregistre rien.
6. **Place restante** (D20E) affichée pendant la séance, pour prévenir avant « carte pleine ». C'est une estimation du boîtier : elle est passée de 35 à 38 après un rebranchement, alors que toutes les photos étaient toujours sur la carte.
7. Fujifilm d'abord (le X-H2 de l'utilisateur ; les boîtiers Fujifilm récents partagent ce protocole). macOS (libgphoto2), les autres marques, le déclenchement depuis Afterdusk et la visée en direct viendront plus tard.

## Conséquences

- Module de connexion au boîtier dans le cœur, avec un boîtier factice pour les tests (aucun boîtier sur les machines d'intégration).
- Test qui vérifie que seules les opérations de la liste peuvent partir vers le boîtier.
- Essais avec le boîtier de l'utilisateur pour chaque étape visible, et mesure du délai entre le déclenchement et l'affichage.
