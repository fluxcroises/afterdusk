/*
 * Couche C minimale au-dessus de LibRaw pour Afterdusk.
 *
 * Elle n'expose que ce dont le cœur Rust a besoin (métadonnées, EXIF,
 * aperçu JPEG intégré) avec des structures à disposition fixe, pour ne pas
 * dépendre de la disposition mémoire de libraw_data_t côté Rust.
 *
 * LibRaw ouvre les fichiers en lecture seule : aucune fonction de cette
 * couche n'écrit sur le fichier source.
 */
#ifndef AFTERDUSK_RAW_H
#define AFTERDUSK_RAW_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ad_raw ad_raw;

typedef struct {
  char make[64];
  char model[64];
  char normalized_make[64];
  char normalized_model[64];
  char software[64];
  char lens_make[128];
  char lens[128];
  char body_serial[64];
  char artist[64];
  float iso_speed;
  float shutter;
  float aperture;
  float focal_len;
  uint16_t focal_len_35mm;
  /* Horodatage converti par LibRaw (mktime en heure locale), 0 si absent. */
  int64_t timestamp;
  uint16_t raw_width;
  uint16_t raw_height;
  uint16_t width;
  uint16_t height;
  int32_t flip;
  uint32_t filters;
  int32_t colors;
  uint32_t dng_version;
  int32_t gps_parsed;
  float gps_latitude[3];
  float gps_longitude[3];
  float gps_altitude;
  char gps_latref;
  char gps_longref;
  char gps_altref;
  int32_t thumb_count;
} ad_raw_info;

/* Ouvre un fichier (chemin UTF-8) et lit ses métadonnées. Renvoie NULL et
 * remplit *error (code LibRaw) en cas d'échec. */
ad_raw* ad_raw_open(const char* utf8_path, int* error);
void ad_raw_close(ad_raw* raw);

const ad_raw_info* ad_raw_get_info(const ad_raw* raw);

/* Balises EXIF/TIFF relevées pendant l'analyse, valeur déjà mise en texte.
 * ifd : 0 = EXIF, 1 = IFD0 TIFF, 4 = GPS. */
size_t ad_raw_exif_count(const ad_raw* raw);
int ad_raw_exif_get(const ad_raw* raw, size_t index, uint32_t* ifd, uint32_t* tag,
                    const char** value);

/* Aperçus intégrés déclarés par le fichier (dimensions parfois nulles).
 * format : code LibRaw_internal_thumbnail_formats (4 = JPEG). */
int ad_raw_thumb_count(const ad_raw* raw);
int ad_raw_thumb_item(const ad_raw* raw, int index, int* format, uint16_t* width,
                      uint16_t* height, uint32_t* length);

/* Extrait l'aperçu `index`. Le tampon reste valide jusqu'au prochain appel ou
 * à ad_raw_close. format : 1 = JPEG, 2 = bitmap RVB 8 bits, 3 = JPEG XL.
 * Renvoie 0 ou un code d'erreur LibRaw. */
int ad_raw_thumbnail(ad_raw* raw, int index, const uint8_t** data, size_t* size, int* format,
                     uint16_t* width, uint16_t* height, uint16_t* colors, uint16_t* bits);

/* Développe l'image, sans aucune retouche de LibRaw (ni courbe ni luminosité
 * automatique) : dématriçage (AHD, ou celui du X-Trans), balance des blancs
 * du boîtier, matrice couleur du boîtier vers XYZ (D65), linéaire, 16 bits,
 * trois canaux, orientation appliquée. half_size : moitié de la définition,
 * pour l'aperçu de retouche. Les données restent valides jusqu'au prochain
 * appel ou à la fermeture. */
int ad_raw_develop(ad_raw* raw, int half_size, const uint16_t** data, uint16_t* width,
                   uint16_t* height);

/* Comme ad_raw_develop ; fast_xtrans : dématriçage X-Trans en une passe au
 * lieu de trois (environ trois fois plus rapide), pour l'aperçu au zoom. Les
 * capteurs de Bayer gardent AHD. */
int ad_raw_develop_ex(ad_raw* raw, int half_size, int fast_xtrans, const uint16_t** data,
                      uint16_t* width, uint16_t* height);

/* Couleur du boîtier, relevée par ad_raw_develop avant que LibRaw n'applique
 * la balance des blancs : multiplicateurs de la prise de vue (cam_mul, nuls
 * ou négatifs s'ils sont absents), multiplicateurs de la lumière du jour
 * (pre_mul, neutre D65) et matrice boîtier normalisé vers sRGB linéaire. */
typedef struct {
  float cam_mul[4];
  float pre_mul[4];
  float rgb_cam[3][4];
  int32_t colors;
} ad_raw_color;

const ad_raw_color* ad_raw_get_color(const ad_raw* raw);

const char* ad_raw_strerror(int error);
const char* ad_raw_version(void);

#ifdef __cplusplus
}
#endif

#endif
