# Audible decrypt

Drive the synthetic AAX/AAXC fixture through the published helper. Do not sign
in to Audible and do not read stored credentials.

```bash
bash scripts/verify.sh decrypt | tee "${EVIDENCE}/decrypt.log"
```

Pass when the lane prints `PASS decrypt`. Copy the last PASS/FAIL block into
`${EVIDENCE}/summary.txt`. There is no `abb-dev` export for this feature; the
engine decrypt tests are the proof.
