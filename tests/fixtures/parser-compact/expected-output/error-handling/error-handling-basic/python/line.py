def parse_score
  try
    assign value
    except ValueError as error
      raise ValueError:invalid
  if value < 0
    raise ValueError:negative
  return value
