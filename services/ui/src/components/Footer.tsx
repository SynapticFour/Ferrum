export function Footer() {
  return (
    <footer className="border-t border-gray-800 mt-auto py-4 px-6">
      <div className="flex items-center justify-between text-xs text-gray-500">
        <span>
          Ferrum · © 2025{' '}
          <a
            href="https://synapticfour.de"
            target="_blank"
            rel="noopener noreferrer"
            className="hover:text-gray-300 transition-colors"
          >
            Synaptic Four
          </a>
        </span>
        <span className="text-center hidden md:block">
          Precise tools for precise science
        </span>
        <span className="text-right">
          Developed in Germany 🇩🇪 by individuals on the autism spectrum
        </span>
      </div>
    </footer>
  );
}
