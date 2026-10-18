import { FileImage } from "lucide-react";
import { useState } from "react";

interface ThumbnailProps {
  src: string | null;
  /** Texte affiché à la place de l'image tant qu'elle n'est pas prête. */
  fallback?: string;
}

/** Image chargée à la demande, avec un repli neutre si elle manque. */
export function Thumbnail({ src, fallback }: ThumbnailProps) {
  const [failed, setFailed] = useState<string | null>(null);
  const showImage = src !== null && failed !== src;

  return (
    <span className="thumbnail" aria-hidden="true">
      {showImage ? (
        <img src={src} alt="" loading="lazy" decoding="async" onError={() => setFailed(src)} />
      ) : (
        <span className="thumbnail-fallback">
          <FileImage strokeWidth={1.25} />
          {fallback && <span className="thumbnail-label">{fallback}</span>}
        </span>
      )}
    </span>
  );
}
