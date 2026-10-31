# Automatic registration through a bootstrap WARP tunnel

Implement the user-approved sequence: direct Cloudflare API, temporary AWG/WG
tunnel using WarpScout's public bootstrap identity, personal registration, normal
server selection. No external relay, proxy settings, or imported user credentials.

1. Add a bounded registration-only mode to the owned Go worker. Reuse its AWG
   netstack, supported tuning profiles and framed pipes. Probe endpoints before
   sending a registration; preserve TLS verification and never replay a POST.
2. Share registration HTTP routing between the Rust WG and MASQUE registrars.
   Retain Rust key generation, pending persistence and API rejection handling.
   Pin and supervise the worker using existing process ownership mechanisms.
3. Reproduce blocked direct access with local HTTP fixtures, verify registration
   forwarding and cancellation, then run Go and targeted Rust tests. Attempt a
   separate live bootstrap registration without modifying system networking.
4. Record observed native results and limits, and add one user-visible changelog
   entry. Do not claim that a local success establishes concurrent-client scale.
